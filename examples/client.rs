use futures::prelude::*;
use serde_json::json;
use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_util::codec::{FramedWrite, LengthDelimitedCodec};

#[tokio::main]
pub async fn main() {
    // Bind a server socket
    let socket = TcpStream::connect("127.0.0.1:17653").await.unwrap();

    // Delimit frames using a length header
    let length_delimited = FramedWrite::new(socket, LengthDelimitedCodec::new());

    // Serialize frames with JSON
    let mut serialized = tokio_serde::FramedWrite::new(length_delimited, Json::default());

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
