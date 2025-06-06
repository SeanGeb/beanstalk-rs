use events::BeanstalkClientEvent;
use protocol::Response;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{self, Framed};

pub mod decoder;
pub mod encoder;
pub mod events;
mod parser;
pub mod protocol;

pub fn framed<T: AsyncRead + AsyncWrite>(stream: T) -> Framed<T, Codec> {
    Framed::new(stream, Default::default())
}

#[derive(Default)]
pub struct Codec {
    d: decoder::Decoder,
    e: encoder::Encoder,
}

impl codec::Decoder for Codec {
    type Item = BeanstalkClientEvent;

    type Error = decoder::Error;

    fn decode(
        &mut self,
        src: &mut bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        self.d.decode(src)
    }
}

impl codec::Encoder<Response> for Codec {
    type Error = encoder::Error;

    fn encode(
        &mut self,
        item: Response,
        dst: &mut bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        self.e.encode(item, dst)
    }
}
