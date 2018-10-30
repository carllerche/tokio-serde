//! This crate provides the utilities needed to easily implement a Tokio
//! transport using [serde] for serialization and deserialization of frame
//! values.
//!
//! **Note**, if you are an end user, you probably won't want to use this crate directly.
//! Instead, use a `tokio-serde-*` crate that implements a specific
//! serialization format. For example [tokio-serde-json] uses [serde-json] to
//! serialize and deserialize frames.
//!
//! # Introduction
//!
//! This crate provides [transport] combinators that transform a stream of
//! frames encoded as bytes into a stream of frame values. It is expected that
//! the framing happens at another layer. One option is to use a [length
//! delimited] framing transport.
//!
//! The crate provides two traits that must be implemented: [`Serializer`] and
//! [`Deserializer`]. Implementations of these traits are then passed to
//! [`FramedRead`] or [`FramedWrite`] along with the upsteram [`Stream`] or
//! [`Sink`] that handles the byte encoded frames.
//!
//! By doing this, a transformation pipeline is built. For reading [json], it looks
//! something like this:
//!
//! * `tokio_serde_json::JsonRead`
//! * `tokio_serde::FramedRead`
//! * `tokio::codec::FramedRead`
//! * `tokio::net::TcpStream`
//!
//! The write half looks like:
//!
//! * `tokio_serde_json::JsonWrite`
//! * `tokio_serde::FramedWrite`
//! * `tokio_io::codec::FramedWrite`
//! * `tokio::net::TcpStream`
//!
//! # Examples
//!
//! For an example, see how [tokio-serde-json] is implemented:
//!
//! * [adapter](https://github.com/carllerche/tokio-serde-json/blob/master/src/lib.rs).
//! * [server](https://github.com/carllerche/tokio-serde-json/blob/master/examples/server.rs)
//! * [client](https://github.com/carllerche/tokio-serde-json/blob/master/examples/client.rs)
//!
//! [serde]: https://serde.rs
//! [tokio-serde-json]: http://github.com/carllerche/tokio-serde-json
//! [serde-json]: https://github.com/serde-rs/json
//! [transport]: https://tokio.rs/docs/going-deeper/transports/
//! [`Serializer`]: trait.Serializer.html
//! [`Deserializer`]: trait.Deserializer.html
//! [`FramedRead`]: struct.FramedRead.html
//! [`FramedWrite`]: trait.FramedWrite.html
//! [json]: http://github.com/carllerche/tokio-serde-json

#[macro_use]
extern crate futures;
extern crate bytes;

mod buffer_one;

use buffer_one::BufferOne;

use bytes::{Bytes, BytesMut};
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};

use std::marker::PhantomData;

/// Serializes a value into a destination buffer
///
/// Implementations of `Serializer` are able to take values of type `T` and
/// convert them to a byte representation. The specific byte format, i.e. JSON,
/// protobuf, binpack, ... is an implementation detail.
///
/// The `serialize` function takes `&mut self`, allowing for `Serializer`
/// instances to be created with runtime configuration settings.
///
/// # Examples
///
/// An integer serializer that allows the width to be configured.
///
/// ```
/// # extern crate tokio_serde;
/// # extern crate bytes;
///
/// use tokio_serde::Serializer;
/// use bytes::{Bytes, BytesMut, BufMut, BigEndian};
///
/// struct IntSerializer {
///     width: usize,
/// }
///
/// #[derive(Debug)]
/// enum Error {
///     Overflow,
/// }
///
/// impl Serializer<u64> for IntSerializer {
///     type Error = Error;
///
///     fn serialize(&mut self, item: &u64) -> Result<Bytes, Self::Error> {
///         assert!(self.width <= 8);
///
///         let max = (1 << (self.width * 8)) - 1;
///
///         if *item > max {
///             return Err(Error::Overflow);
///         }
///
///         let mut ret = BytesMut::with_capacity(self.width);
///         ret.put_uint::<BigEndian>(*item, self.width);
///         Ok(ret.into())
///     }
/// }
///
/// # pub fn main() {
/// let mut serializer = IntSerializer { width: 3 };
///
/// let buf = serializer.serialize(&5).unwrap();
/// assert_eq!(buf, &b"\x00\x00\x05"[..]);
/// # }
/// ```
pub trait Serializer<T> {
    type Error;

    /// Serializes `item` into a new buffer
    ///
    /// The serialization format is specific to the various implementations of
    /// `Serializer`. If the serialization is successful, a buffer containing
    /// the serialized item is returned. If the serialization is unsuccessful,
    /// an error is returned.
    ///
    /// Implementations of this function should not mutate `item` via any sort
    /// of internal mutability strategy.
    ///
    /// See the trait level docs for more detail.
    fn serialize(&mut self, item: &T) -> Result<Bytes, Self::Error>;
}

/// Deserializes a value from a source buffer
///
/// Implementatinos of `Deserializer` take a byte buffer and return a value by
/// parsing the contents of the buffer according to the implementation's format.
/// The specific byte format, i.e. JSON, protobuf, binpack, is an implementation
/// detail
///
/// The `deserialize` function takes `&mut self`, allowing for `Deserializer`
/// instances to be created with runtime configuration settings.
///
/// It is expected that the supplied buffer represents a full value and only
/// that value. If after deserializing a value there are remaining bytes the
/// buffer, the deserializer will return an error.
///
/// # Examples
///
/// An integer deserializer that allows the width to be configured.
///
/// ```
/// # extern crate tokio_serde;
/// # extern crate bytes;
///
/// use tokio_serde::Deserializer;
/// use bytes::{BytesMut, IntoBuf, Buf, BigEndian};
///
/// struct IntDeserializer {
///     width: usize,
/// }
///
/// #[derive(Debug)]
/// enum Error {
///     Underflow,
///     Overflow
/// }
///
/// impl Deserializer<u64> for IntDeserializer {
///     type Error = Error;
///
///     fn deserialize(&mut self, buf: &BytesMut) -> Result<u64, Self::Error> {
///         assert!(self.width <= 8);
///
///         if buf.len() > self.width {
///             return Err(Error::Overflow);
///         }
///
///         if buf.len() < self.width {
///             return Err(Error::Underflow);
///         }
///
///         let ret = buf.into_buf().get_uint::<BigEndian>(self.width);
///         Ok(ret)
///     }
/// }
///
/// # pub fn main() {
/// let mut deserializer = IntDeserializer { width: 3 };
///
/// let i = deserializer.deserialize(&b"\x00\x00\x05"[..].into()).unwrap();
/// assert_eq!(i, 5);
/// # }
/// ```
pub trait Deserializer<T> {
    type Error;

    /// Deserializes a value from `buf`
    ///
    /// The serialization format is specific to the various implementations of
    /// `Deserializer`. If the deserialization is successful, the value is
    /// returned. If the deserialization is unsuccessful, an error is returned.
    ///
    /// See the trait level docs for more detail.
    fn deserialize(&mut self, src: &BytesMut) -> Result<T, Self::Error>;
}

/// Adapts a stream of buffers to a stream of values by deserializing them.
///
/// It is expected that the buffers yielded by the supplied stream be framed. In
/// other words, each yielded buffer must represent exactly one serialized
/// value. The specific framing strategy is left up to the implementor. One option
/// would be to use [length_delimited] provided by [tokio-io].
///
/// [length_delimited]: http://docs.rs/tokio-io/codec/length_delimited/index.html
/// [tokio-io]: http://crates.io/crates/tokio-io
pub struct FramedRead<T, U, S> {
    inner: T,
    deserializer: S,
    item: PhantomData<U>,
}

/// Adapts a buffer sink to a value sink by serializing the values.
///
/// The provided buffer sink will received buffer values containing the
/// serialized value. Each buffer contains exactly one value. This sink will be
/// responsible for writing these buffers to an `AsyncWrite` using some sort of
/// framing strategy. The specific framing strategy is left up to the
/// implementor. One option would be to use [length_delimited] provided by
/// [tokio-io].
///
/// [length_delimited]: http://docs.rs/tokio-io/codec/length_delimited/index.html
/// [tokio-io]: http://crates.io/crates/tokio-io
pub struct FramedWrite<T, U, S>
where
    T: Sink,
{
    inner: BufferOne<T>,
    serializer: S,
    item: PhantomData<U>,
}

// ===== impl FramedRead =====

impl<T, U, S> FramedRead<T, U, S>
where
    T: Stream,
    BytesMut: From<T::Item>,
    S: Deserializer<U>,
    S::Error: Into<T::Error>,
{
    /// Creates a new `FramedRead` with the given buffer stream and deserializer.
    pub fn new(inner: T, deserializer: S) -> FramedRead<T, U, S> {
        FramedRead {
            inner: inner,
            deserializer: deserializer,
            item: PhantomData,
        }
    }
}

impl<T, U, S> FramedRead<T, U, S> {
    /// Returns a reference to the underlying stream wrapped by `FramedRead`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Returns a mutable reference to the underlying stream wrapped by
    /// `FramedRead`.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise
    /// being worked with.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consumes the `FramedRead`, returning its underlying stream.
    ///
    /// Note that care should be taken to not tamper with the underlying stream
    /// of data coming in as it may corrupt the stream of frames otherwise being
    /// worked with.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, U, S> Stream for FramedRead<T, U, S>
where
    T: Stream,
    T::Error: From<S::Error>,
    BytesMut: From<T::Item>,
    S: Deserializer<U>,
{
    type Item = U;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match try_ready!(self.inner.poll()) {
            Some(bytes) => {
                let val = try!(self.deserializer.deserialize(&bytes.into()));
                Ok(Async::Ready(Some(val)))
            }
            None => Ok(Async::Ready(None)),
        }
    }
}

impl<T, U, S> FramedWrite<T, U, S>
where
    T: Sink<SinkItem = Bytes>,
    S: Serializer<U>,
    S::Error: Into<T::SinkError>,
{
    /// Creates a new `FramedWrite` with the given buffer sink and serializer.
    pub fn new(inner: T, serializer: S) -> Self {
        FramedWrite {
            inner: BufferOne::new(inner),
            serializer: serializer,
            item: PhantomData,
        }
    }
}

impl<T: Sink, U, S> FramedWrite<T, U, S> {
    /// Returns a reference to the underlying sink wrapped by `FramedWrite`.
    ///
    /// Note that care should be taken to not tamper with the underlying sink as
    /// it may corrupt the sequence of frames otherwise being worked with.
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Returns a mutable reference to the underlying sink wrapped by
    /// `FramedWrite`.
    ///
    /// Note that care should be taken to not tamper with the underlying sink as
    /// it may corrupt the sequence of frames otherwise being worked with.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Consumes the `FramedWrite`, returning its underlying sink.
    ///
    /// Note that care should be taken to not tamper with the underlying sink as
    /// it may corrupt the sequence of frames otherwise being worked with.
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

impl<T, U, S> Sink for FramedWrite<T, U, S>
where
    T: Sink<SinkItem = Bytes>,
    S: Serializer<U>,
    S::Error: Into<T::SinkError>,
{
    type SinkItem = U;
    type SinkError = T::SinkError;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        if !self.inner.poll_ready().is_ready() {
            return Ok(AsyncSink::NotReady(item));
        }

        let res = self.serializer.serialize(&item);
        let bytes = try!(res.map_err(Into::into));

        assert!(try!(self.inner.start_send(bytes)).is_ready());

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_complete()
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        try_ready!(self.poll_complete());
        self.inner.close()
    }
}
