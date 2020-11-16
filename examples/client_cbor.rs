use futures::prelude::*;
use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_util::codec::{FramedWrite, LengthDelimitedCodec};

type CborWriter = tokio_serde::Framed<tokio_util::codec::FramedWrite<tokio::net::TcpStream, tokio_util::codec::LengthDelimitedCodec>, serde_cbor::value::Value, serde_cbor::value::Value, tokio_serde::formats::Cbor<serde_cbor::value::Value,serde_cbor::value::Value>>;

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

    // Delimit frames using a length header
    let length_delimited = FramedWrite::new(socket, LengthDelimitedCodec::new());

    // Serialize frames with JSON
    //let mut serialized = tokio_serde::SymmetricallyFramed::new(length_delimited, SymmetricalCbor::default());
    let mut serialized = setup_writer(socket);


    // Send the value
    serialized
        .send(vec![1i32,2,3])    // but, Vec does match serde_cbor::value::Value!
        .await
        .unwrap()
}
