use futures::prelude::*;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_serde::formats::*;
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

#[tokio::main]
pub async fn main() {
    // Bind a server socket
    let mut listener = TcpListener::bind("127.0.0.1:17653").await.unwrap();

    println!("listening on {:?}", listener.local_addr());

    while let Some(socket) = listener.try_next().await.unwrap() {
        // Delimit frames using a length header
        let length_delimited = FramedRead::new(socket, LengthDelimitedCodec::new());

        // Deserialize frames
        let mut deserialized = tokio_serde::SymmetricallyFramed::new(
            length_delimited,
            SymmetricalJson::<Value>::default(),
        );

        // Spawn a task that prints all received messages to STDOUT
        tokio::spawn(async move {
            while let Some(msg) = deserialized.try_next().await.unwrap() {
                println!("GOT: {:?}", msg);
            }
        });
    }
}
