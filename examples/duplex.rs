use futures::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio_serde::formats::*;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Debug, Serialize, Deserialize)]
enum Request {
    Hello(String),
}

#[derive(Debug, Serialize, Deserialize)]
enum Response {
    Greeting(String),
}

pub async fn server(name: &'static str) {
    let listener = TcpListener::bind("127.0.0.1:17653").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();

        // Delimit frames using a length header
        let length_delimited = Framed::new(socket, LengthDelimitedCodec::new());

        // Serialize and deserialize frames with JSON
        let mut stream: tokio_serde::Framed<_, Request, Response, _> =
            tokio_serde::Framed::new(length_delimited, Json::<Request, Response>::default());

        tokio::spawn(async move {
            while let Some(msg) = stream.try_next().await.unwrap() {
                println!("Server GOT: {:?})", msg);

                stream
                    .send(Response::Greeting(name.to_owned()))
                    .await
                    .unwrap();
            }
        });
    }
}

pub async fn client(name: &str) {
    // Bind a server socket
    let socket = TcpStream::connect("127.0.0.1:17653").await.unwrap();

    // Delimit frames using a length header
    let length_delimited = Framed::new(socket, LengthDelimitedCodec::new());

    // Serialize and deserialize frames with JSON
    let mut stream =
        tokio_serde::Framed::new(length_delimited, Json::<Response, Request>::default());

    // Send the value
    stream
        .send(Request::Hello(name.to_owned()))
        .await
        .unwrap();

    if let Some(msg) = stream.try_next().await.unwrap() {
        println!("Client GOT: {:?})", msg);
    }
}

#[tokio::main]
pub async fn main() {
    let server = server("server");
    let client_1 = client("client1");
    let client_2 = client("client2");

    futures::join!(server, client_1, client_2);
}
