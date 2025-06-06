use std::{error, fmt, io};

use bytes::Buf;
use itertools::Itertools;
use tokio_util::codec;

use super::events::BeanstalkClientEvent;
use super::protocol::{Command, Response};

/// A decoder for a stream of Beanstalk protocol client messages.
///
/// **Compatability note**: there is an important and intentional behaviour
/// difference compared to the original beanstalkd.
///
/// After encountering certain classes of client error that suggest the client
/// and server are out-of-sync at the protocol level, an unrecoverable framing
/// error is returned.
///
/// This should not affect well-behaved clients, but misbehaving clients will be
/// disconnected.
#[derive(Debug, Default)]
pub enum Decoder {
    #[default]
    ParseCommand,
    ParseJob {
        remaining: usize,
    },
    DiscardToNewline,
}

impl codec::Decoder for Decoder {
    type Item = BeanstalkClientEvent;

    type Error = Error;

    fn decode(
        &mut self,
        src: &mut bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        match *self {
            Decoder::ParseCommand => {
                // Grab up to 224 bytes of \r\n-terminated command
                // Imagine if src contains b"abc\r\n": this generates tuples
                // ab, bc, c\r, \r\n, so idx is 3.
                // Note also that idx != None implies src.len() >= idx + 2.
                match src
                    .iter()
                    .take(224) // inspect at most 224 bytes
                    .tuple_windows() // in pairs
                    .find_position(|&(&a, &b)| a == b'\r' && b == b'\n')
                {
                    Some((idx, _)) => {
                        // Panic safety: split_to panics unless src.len() >= idx.
                        let cmd = src.split_to(idx);
                        // Panic safety: advance panics unless src.len() >= 2,
                        // which from the previous means we want src.len() >= idx + 2,
                        // which is guaranteed by the form of the iterator.
                        src.advance(2); // discards the \r\n left in the buffer

                        let cmd: Command = cmd.as_ref().try_into()?;

                        if let Command::Put { n_bytes, .. } = cmd {
                            let n_bytes = n_bytes as usize;

                            // Reserve up to MAX_BUFFER_RESERVATION bytes to
                            // reduce re-allocations while accumulating the job
                            // TODO: this should be assigned based on the max
                            // size of a job before it spills to disk, plus \r\n
                            src.reserve(n_bytes.min(16_384));
                            *self = Self::ParseJob { remaining: n_bytes };
                        }

                        Ok(Some(Self::Item::Command(cmd)))
                    },
                    None => {
                        if src.len() >= 224 {
                            *self = Self::DiscardToNewline;
                            Err(Response::BadFormat.into())
                        } else {
                            // If < 224 bytes, we may get a \r\n next time
                            Ok(None)
                        }
                    },
                }
            },
            Decoder::ParseJob { remaining: 0 } => {
                // We've taken as many bytes as the client said to expect, so we
                // need to check for and consume an \r\n.
                if src.len() < 2 {
                    return Ok(None);
                }

                // Panic safety: indexing panics unless src.len() >= 2, which
                // we've just asserted.
                if src[0] == b'\r' && src[1] == b'\n' {
                    src.advance(2);
                    *self = Self::ParseCommand;
                    Ok(Some(Self::Item::PutEnd))
                } else {
                    *self = Self::DiscardToNewline;
                    Err(Response::ExpectedCRLF.into())
                }
            },
            Decoder::ParseJob { remaining } => {
                // NB: remaining > 0 as the previous condition didn't match
                if src.len() == 0 {
                    // Ensures a PutChunk always contains at least one byte of
                    // data, and causes the codec to return an error if we're
                    // trying to read a job and reach the end of stream
                    // prematurely.
                    return Ok(None);
                }

                let take_len = remaining.min(src.len());

                // Panic safety: remaining - take_len cannot be negative, but
                // this is assured as take_len == min(remaining, src.len())
                // ==> take_len <= remaining && take_len <= src.len()
                *self = Self::ParseJob {
                    remaining: remaining - take_len,
                };

                // Panic safety: split_to panics unless take_len <= src.len(),
                // which is assured above.
                return Ok(Some(Self::Item::PutChunk(
                    src.split_to(take_len).freeze(),
                )));
            },
            Decoder::DiscardToNewline => {
                if src.len() == 0 {
                    return Ok(None);
                }

                // If we can find a \r\n, discard up to and including it
                if let Some((idx, _)) = src
                    .iter()
                    .tuple_windows()
                    .find_position(|&(&a, &b)| a == b'\r' && b == b'\n')
                {
                    // Remove everything up to and including the \r\n, then try
                    // to return to normal parsing.
                    // Panic safety: advance panics unless src.len() >= idx + 2,
                    // which is guaranteed by the find_position call succeeding.
                    src.advance(idx + 2);
                    *self = Self::ParseCommand;
                } else {
                    // Preserve the last byte in case it's \r
                    // Panic safety: src.len() - 1 can't be negative, but we've
                    // already asserted src.len() != 0 so this is safe.
                    // Panic safety: advance panics unless
                    // src.len() >= src.len() - 1, which is guaranteed.
                    src.advance(src.len() - 1);
                }

                // Ok(None) not suitable here due to end of stream semantics
                Ok(Some(Self::Item::Discarded))
            },
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Client(Response),
    IO(io::Error),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<Response> for Error {
    fn from(value: Response) -> Self {
        Self::Client(value)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IO(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use io::ErrorKind;
    use tokio_stream::StreamExt;
    use tokio_util::codec::FramedRead;

    // helpers
    fn cmd(c: Command) -> BeanstalkClientEvent {
        BeanstalkClientEvent::Command(c)
    }
    fn chunk(c: &[u8]) -> BeanstalkClientEvent {
        BeanstalkClientEvent::PutChunk(c.to_owned().into())
    }
    fn stream_from(cmds: &[&str]) -> Vec<u8> {
        let mut stream = cmds.join("\r\n");
        stream.push_str("\r\n");
        stream.into_bytes()
    }

    // Test a normal sequence of commands, including puts
    #[tokio::test]
    async fn test_normal() {
        let stream = stream_from(&[
            "use tube-1",
            "use tube-2",
            "put 10000 0 60 8",
            "abcdefgh",
            "use tube-3",
            "put 10001 1 61 7",
            "0000000",
            "put 10002 2 62 6",
            "11\r\n11",
            "quit",
        ]);

        let expect = [
            cmd(Command::Use {
                tube: b"tube-1".into(),
            }),
            cmd(Command::Use {
                tube: b"tube-2".into(),
            }),
            cmd(Command::Put {
                pri: 10000,
                delay: 0,
                ttr: 60,
                n_bytes: 8,
            }),
            chunk(b"abcdefgh"),
            BeanstalkClientEvent::PutEnd,
            cmd(Command::Use {
                tube: b"tube-3".into(),
            }),
            cmd(Command::Put {
                pri: 10001,
                delay: 1,
                ttr: 61,
                n_bytes: 7,
            }),
            chunk(b"0000000"),
            BeanstalkClientEvent::PutEnd,
            cmd(Command::Put {
                pri: 10002,
                delay: 2,
                ttr: 62,
                n_bytes: 6,
            }),
            chunk(b"11\r\n11"),
            BeanstalkClientEvent::PutEnd,
            cmd(Command::Quit),
        ];

        let decoder: Decoder = Default::default();
        let mut framed = FramedRead::new(stream.as_ref(), decoder);

        for evt in expect {
            let got = framed.next().await;
            assert_eq!(got.unwrap().unwrap(), evt);
        }

        // End of stream should be OK
        assert!(framed.next().await.is_none());
    }

    // Test an early EOS during a put command
    #[tokio::test]
    async fn test_put_eos() {
        let stream = stream_from(&[
            "put 10000 0 60 8",
            "abcde", // three bytes short
        ]);

        let decoder: Decoder = Default::default();
        let mut framed = FramedRead::new(stream.as_ref(), decoder);

        assert_eq!(
            framed.next().await.unwrap().unwrap(),
            cmd(Command::Put {
                pri: 10000,
                delay: 0,
                ttr: 60,
                n_bytes: 8,
            }),
        );
        assert_eq!(framed.next().await.unwrap().unwrap(), chunk(b"abcde\r\n"));

        assert!(framed.next().await.is_none());
    }

    // Test an early EOS with an incomplete command
    #[tokio::test]
    async fn test_eos() {
        let stream: Vec<u8> = b"use bar\r\nuse foo".into();

        let decoder: Decoder = Default::default();
        let mut framed = FramedRead::new(stream.as_ref(), decoder);

        assert_eq!(
            framed.next().await.unwrap().unwrap(),
            cmd(Command::Use {
                tube: b"bar".into()
            }),
        );

        if let Error::IO(err) = framed.next().await.unwrap().unwrap_err() {
            assert_eq!(err.kind(), ErrorKind::Other);
            let inner = err.into_inner().unwrap();
            assert_eq!(format!("{inner}"), "bytes remaining on stream");
        } else {
            panic!("expected CodecError::IO, got other");
        }

        assert!(framed.next().await.is_none());
    }

    // Test recovery when discarding to the next \r\n
    #[tokio::test]
    async fn test_recovery() {
        // trigger recovery by not ending a job with \r\n
        let stream: Vec<u8> =
            b"put 10000 0 60 4\r\n****stats-tube\r\nuse bar\r\nuse baz\r\n"
                .into();

        let decoder: Decoder = Default::default();
        let mut framed = FramedRead::new(stream.as_ref(), decoder);

        assert_eq!(
            framed.next().await.unwrap().unwrap(),
            cmd(Command::Put {
                pri: 10000,
                delay: 0,
                ttr: 60,
                n_bytes: 4,
            })
        );
        assert_eq!(framed.next().await.unwrap().unwrap(), chunk(b"****"));

        assert!(matches!(
            framed.next().await.unwrap(),
            Err(Error::Client(Response::ExpectedCRLF))
        ));

        assert!(framed.next().await.is_none());
        assert_eq!(
            framed.next().await.unwrap().unwrap(),
            BeanstalkClientEvent::Discarded
        );

        assert_eq!(
            framed.next().await.unwrap().unwrap(),
            cmd(Command::Use {
                tube: b"bar".into()
            })
        );
        assert_eq!(
            framed.next().await.unwrap().unwrap(),
            cmd(Command::Use {
                tube: b"baz".into()
            })
        );

        assert!(framed.next().await.is_none());
    }
}
