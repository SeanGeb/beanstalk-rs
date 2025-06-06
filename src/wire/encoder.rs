use std::{error, fmt, io};

use bytes::BufMut;
use serde::ser;
use tokio_util::codec;

use super::protocol::Response;

// An encoder to produce Beanstalk client messages
#[derive(Debug, Default)]
pub struct Encoder {}

impl codec::Encoder<Response> for Encoder {
    type Error = Error;

    fn encode(
        &mut self,
        item: Response,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        use Response::*;

        fn put_ok_and_data(
            dst: &mut bytes::BytesMut,
            data: impl ser::Serialize,
        ) -> serde_yaml::Result<()> {
            //! Serialises data into dst as `OK {data.len()}\r\n{data}\r\n`.
            //! On serialisation failure, sends `InternalError` to the client
            //! and returns the error.
            match serde_yaml::to_string(&data) {
                Ok(data) => {
                    let data = data.into_bytes();

                    let len_str = data.len().to_string().into_bytes();
                    // "OK {len}\r\n{data}\r\n"
                    dst.reserve(3 + len_str.len() + 2 + data.len() + 2);

                    dst.put_slice(b"OK ");
                    dst.extend(len_str);
                    dst.put_slice(b"\r\n");
                    dst.extend(data);
                    dst.put_slice(b"\r\n");

                    Ok(())
                },
                Err(err) => {
                    dst.put_slice(b"INTERNAL_ERROR\r\n");
                    Err(err)
                },
            }
        }

        fn put_str_and_u32(
            dst: &mut bytes::BytesMut,
            str: &[u8],
            num: u32,
        ) -> () {
            //! Writes `"{str} {num}\r\n"` to `dst`
            let num_str = num.to_string().into_bytes();
            // "{str} {num}\r\n"
            dst.reserve(str.len() + 1 + num_str.len() + 2);

            dst.put_slice(str);
            dst.put_slice(b" ");
            dst.extend(num_str);
            dst.put_slice(b"\r\n");
        }

        fn put_str_and_u64(
            dst: &mut bytes::BytesMut,
            str: &[u8],
            num: u64,
        ) -> () {
            //! Writes `"{str} {num}\r\n"` to `dst`
            let num_str = num.to_string().into_bytes();
            // "{str} {num}\r\n"
            dst.reserve(str.len() + 1 + num_str.len() + 2);

            dst.put_slice(str);
            dst.put_slice(b" ");
            dst.extend(num_str);
            dst.put_slice(b"\r\n");
        }

        Ok(match item {
            BadFormat => dst.put_slice(b"BAD_FORMAT\r\n"),
            Buried => dst.put_slice(b"BURIED\r\n"),
            DeadlineSoon => dst.put_slice(b"DEADLINE_SOON\r\n"),
            Deleted => dst.put_slice(b"DELETED\r\n"),
            Draining => dst.put_slice(b"DRAINING\r\n"),
            ExpectedCRLF => dst.put_slice(b"EXPECTED_CRLF\r\n"),
            InternalError => dst.put_slice(b"INTERNAL_ERROR\r\n"),
            JobTooBig => dst.put_slice(b"JOB_TOO_BIG\r\n"),
            Kicked => dst.put_slice(b"KICKED\r\n"),
            NotFound => dst.put_slice(b"NOT_FOUND\r\n"),
            NotIgnored => dst.put_slice(b"NOT_IGNORED\r\n"),
            OutOfMemory => dst.put_slice(b"OUT_OF_MEMORY\r\n"),
            Paused => dst.put_slice(b"PAUSED\r\n"),
            Released => dst.put_slice(b"RELEASED\r\n"),
            TimedOut => dst.put_slice(b"TIMED_OUT\r\n"),
            Touched => dst.put_slice(b"TOUCHED\r\n"),
            UnknownCommand => dst.put_slice(b"UNKNOWN_COMMAND\r\n"),

            BuriedID { id } => put_str_and_u64(dst, b"BURIED", id),
            Inserted { id } => put_str_and_u64(dst, b"INSERTED", id),
            KickedCount { count } => put_str_and_u64(dst, b"KICKED", count),
            Watching { count } => put_str_and_u32(dst, b"WATCHING", count),

            OkStatsJob { data } => put_ok_and_data(dst, data)?,
            OkStats { data } => put_ok_and_data(dst, data)?,
            OkListTubes { tubes } => put_ok_and_data(dst, tubes)?,
            OkStatsTube { data } => put_ok_and_data(dst, data)?,

            Using { tube } => {
                // "USING {tube}\r\n"
                dst.reserve(6 + tube.len() + 2);

                dst.put_slice(b"USING ");
                dst.extend(tube);
                dst.put_slice(b"\r\n");
            },

            Reserved { id } => put_str_and_u64(dst, b"RESERVED", id),
            Found { id } => put_str_and_u64(dst, b"FOUND", id),
            JobChunk(data) => dst.extend(data),
            JobEnd => dst.put_slice(b"\r\n"),
        })
    }
}

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Serde(serde_yaml::Error),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(value: serde_yaml::Error) -> Self {
        Self::Serde(value)
    }
}
