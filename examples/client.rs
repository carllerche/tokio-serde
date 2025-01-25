use futures::prelude::*;
use serde_json::json;
use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_util::codec::{FramedWrite, LengthDelimitedCodec};

type JsonWriter = tokio_serde::Framed<tokio_util::codec::FramedWrite<tokio::net::TcpStream, tokio_util::codec::LengthDelimitedCodec>, serde_json::Value, serde_json::Value, tokio_serde::formats::Json<serde_json::Value, serde_json::Value>>;

fn setup_writer(socket: tokio::net::TcpStream) -> JsonWriter {
    // Delimit frames using a length header
    let length_delimited = FramedWrite::new(socket, LengthDelimitedCodec::new());

    // Serialize frames with JSON
    let serialized = tokio_serde::SymmetricallyFramed::new(length_delimited, SymmetricalJson::default());

    return serialized;
}

#[tokio::main]
pub async fn main() {
    // Bind a server socket
    let socket = TcpStream::connect("127.0.0.1:17653").await.unwrap();

    let mut serialized = setup_writer(socket);

    // Send the value
    serialized
        .send(json!({
            "name": "John Doe",
            "age": 43,
            "phones": [
                "+44 1234567",
                "+44 2345678"
            ]
        }))
        .await
        .unwrap()
}
