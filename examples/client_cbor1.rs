use futures::prelude::*;
use serde_cbor;
use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_util::codec::{FramedWrite, LengthDelimitedCodec};

type CborWriter = tokio_serde::Framed<tokio_util::codec::FramedWrite<tokio::net::TcpStream, tokio_util::codec::LengthDelimitedCodec>, serde_cbor::Value, serde_cbor::Value, tokio_serde::formats::Cbor<serde_cbor::Value, serde_cbor::Value>>;

// something like this has been suggested, but so far no luck.
// fn setup_writer(socket: tokio::net::TcpStream) -> impl Sink<??>+Debug {

fn setup_writer(socket: tokio::net::TcpStream) -> CborWriter {
    // Delimit frames using a length header
    let length_delimited = FramedWrite::new(socket, LengthDelimitedCodec::new());

    // Serialize frames with CBOR
    let serialized = tokio_serde::SymmetricallyFramed::new(length_delimited, SymmetricalCbor::default());

    return serialized;
}

#[tokio::main]
pub async fn main() {
    // Bind a server socket
    let socket = TcpStream::connect("127.0.0.1:17653").await.unwrap();

    // Serialize frames with CBOR
    let mut serialized = setup_writer(socket);

    // Send the value
    serialized
        .send(serde_cbor::Value::Array(vec![serde_cbor::Value::Integer(1i128),
                                            serde_cbor::Value::Integer(2i128),
                                            serde_cbor::Value::Integer(3i128)]))
        .await
        .unwrap()
}
