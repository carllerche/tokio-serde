#[macro_use]
extern crate futures;
extern crate tokio_io;
extern crate bytes;

mod buffer_one;

use buffer_one::BufferOne;

use futures::{Async, AsyncSink, StartSend, Sink, Stream, Poll};
use bytes::{Bytes, BytesMut};

use std::io;
use std::marker::PhantomData;

/// Serializes a value into a destination buffer
pub trait Serializer<T> {
    type Error;

    fn serialize(&mut self, item: &T, dst: &BytesMut) -> Result<(), Self::Error>;
}

/// Deserializes a value from a source buffer
pub trait Deserializer<T> {
    type Error;

    fn deserialize(&mut self, src: &Bytes) -> Result<T, Self::Error>;
}

pub struct FramedRead<T, U, S> {
    inner: T,
    deserializer: S,
    item: PhantomData<U>,
}

pub struct FramedWrite<T, U, S> where T: Sink {
    inner: BufferOne<T>,
    serializer: S,
    item: PhantomData<U>,
}

// ===== impl FramedRead =====

impl<T, U, S> FramedRead<T, U, S>
    where T: Stream<Error = io::Error>,
          Bytes: From<T::Item>,
          S: Deserializer<U>,
          S::Error: From<io::Error>,
{
    pub fn new(inner: T, deserializer: S) -> FramedRead<T, U, S> {
        FramedRead {
            inner: inner,
            deserializer: deserializer,
            item: PhantomData,
        }
    }
}

impl<T, U, S> FramedRead<T, U, S> {
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, U, S> Stream for FramedRead<T, U, S>
    where T: Stream<Error = io::Error>,
          Bytes: From<T::Item>,
          S: Deserializer<U>,
          S::Error: From<io::Error>,
{
    type Item = U;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<U>, S::Error> {
        match try_ready!(self.inner.poll()) {
            Some(bytes) => {
                let val = try!(self.deserializer.deserialize(&bytes.into()));
                Ok(Async::Ready(Some(val)))
            }
            None => Ok(Async::Ready(None)),
        }
    }
}

impl<T, U, S> Sink for FramedWrite<T, U, S>
    where T: Sink<SinkItem = BytesMut, SinkError = io::Error>,
          S: Serializer<U>,
          S::Error: Into<io::Error>,
{
    type SinkItem = U;
    type SinkError = io::Error;

    fn start_send(&mut self, item: U) -> StartSend<U, io::Error> {
        if !self.inner.poll_ready().is_ready() {
            return Ok(AsyncSink::NotReady(item));
        }

        // Let the `Serializer` impl reserve the necessary capacity
        let mut bytes = BytesMut::with_capacity(0);

        let res = self.serializer.serialize(&item, &mut bytes);
        try!(res.map_err(Into::into));

        assert!(try!(self.inner.start_send(bytes)).is_ready());

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        self.inner.poll_complete()
    }

    fn close(&mut self) -> Poll<(), io::Error> {
        try_ready!(self.poll_complete());
        self.inner.close()
    }
}
