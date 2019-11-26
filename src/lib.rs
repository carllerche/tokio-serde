//! This crate provides the utilities needed to easily implement a Tokio
//! transport using [serde] for serialization and deserialization of frame
//! values.
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
//! [`FramedRead`] or [`FramedWrite`] along with the upstream [`Stream`] or
//! [`Sink`] that handles the byte encoded frames.
//!
//! By doing this, a transformation pipeline is built. For reading [json], it looks
//! something like this:
//!
//! * `tokio_serde::FramedRead`
//! * `tokio::codec::FramedRead`
//! * `tokio::net::TcpStream`
//!
//! The write half looks like:
//!
//! * `tokio_serde::FramedWrite`
//! * `tokio_io::codec::FramedWrite`
//! * `tokio::net::TcpStream`
//!
//! # Examples
//!
//! For an example, see how JSON support is implemented:
//!
//! * [server](https://github.com/carllerche/tokio-serde/blob/master/examples/server.rs)
//! * [client](https://github.com/carllerche/tokio-serde/blob/master/examples/client.rs)
//!
//! [serde]: https://serde.rs
//! [serde-json]: https://github.com/serde-rs/json
//! [transport]: https://tokio.rs/docs/going-deeper/transports/
//! [`Serializer`]: trait.Serializer.html
//! [`Deserializer`]: trait.Deserializer.html
//! [`FramedRead`]: struct.FramedRead.html
//! [`FramedWrite`]: trait.FramedWrite.html

use {
    bytes::{Bytes, BytesMut},
    futures::{prelude::*, ready},
    pin_project::pin_project,
    std::{
        marker::PhantomData,
        pin::Pin,
        task::{Context, Poll},
    },
};

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
/// use tokio_serde::Serializer;
/// use bytes::{Buf, Bytes, BytesMut, BufMut};
/// use std::pin::Pin;
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
///     fn serialize(self: Pin<&mut Self>, item: &u64) -> Result<Bytes, Self::Error> {
///         assert!(self.width <= 8);
///
///         let max = (1 << (self.width * 8)) - 1;
///
///         if *item > max {
///             return Err(Error::Overflow);
///         }
///
///         let mut ret = BytesMut::with_capacity(self.width);
///         ret.put_uint(*item, self.width);
///         Ok(ret.to_bytes())
///     }
/// }
///
/// let mut serializer = IntSerializer { width: 3 };
///
/// let buf = Pin::new(&mut serializer).serialize(&5).unwrap();
/// assert_eq!(buf, &b"\x00\x00\x05"[..]);
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
    fn serialize(self: Pin<&mut Self>, item: &T) -> Result<Bytes, Self::Error>;
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
/// use tokio_serde::Deserializer;
/// use bytes::{BytesMut, Buf};
/// use std::pin::Pin;
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
///     fn deserialize(self: Pin<&mut Self>, buf: &BytesMut) -> Result<u64, Self::Error> {
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
///         let ret = std::io::Cursor::new(buf).get_uint(self.width);
///         Ok(ret)
///     }
/// }
///
/// let mut deserializer = IntDeserializer { width: 3 };
///
/// let i = Pin::new(&mut deserializer).deserialize(&b"\x00\x00\x05"[..].into()).unwrap();
/// assert_eq!(i, 5);
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
    fn deserialize(self: Pin<&mut Self>, src: &BytesMut) -> Result<T, Self::Error>;
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
#[pin_project]
pub struct FramedRead<T, U, S> {
    #[pin]
    inner: T,
    #[pin]
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
#[pin_project]
pub struct FramedWrite<T, U, S> {
    #[pin]
    inner: T,
    #[pin]
    serializer: S,
    item: PhantomData<U>,
}

impl<T, U, S> FramedRead<T, U, S> {
    /// Creates a new `FramedRead` with the given buffer stream and deserializer.
    pub fn new(inner: T, deserializer: S) -> FramedRead<T, U, S> {
        FramedRead {
            inner,
            deserializer,
            item: PhantomData,
        }
    }

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
    T: TryStream<Ok = BytesMut>,
    T::Error: From<S::Error>,
    BytesMut: From<T::Ok>,
    S: Deserializer<U>,
{
    type Item = Result<U, T::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!(self.as_mut().project().inner.try_poll_next(cx)) {
            Some(bytes) => Poll::Ready(Some(Ok(self
                .as_mut()
                .project()
                .deserializer
                .deserialize(&bytes?)?))),
            None => Poll::Ready(None),
        }
    }
}

impl<T, U, S, SinkItem> Sink<SinkItem> for FramedRead<T, U, S>
where
    T: Sink<SinkItem>,
{
    type Error = T::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: SinkItem) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}

impl<T, U, S> FramedWrite<T, U, S> {
    /// Creates a new `FramedWrite` with the given buffer sink and serializer.
    pub fn new(inner: T, serializer: S) -> Self {
        FramedWrite {
            inner,
            serializer,
            item: PhantomData,
        }
    }

    /// Returns a reference to the underlying sink wrapped by `FramedWrite`.
    ///
    /// Note that care should be taken to not tamper with the underlying sink as
    /// it may corrupt the sequence of frames otherwise being worked with.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Returns a mutable reference to the underlying sink wrapped by
    /// `FramedWrite`.
    ///
    /// Note that care should be taken to not tamper with the underlying sink as
    /// it may corrupt the sequence of frames otherwise being worked with.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consumes the `FramedWrite`, returning its underlying sink.
    ///
    /// Note that care should be taken to not tamper with the underlying sink as
    /// it may corrupt the sequence of frames otherwise being worked with.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, U, S> Stream for FramedWrite<T, U, S>
where
    T: Stream,
{
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

impl<T, U, S> Sink<U> for FramedWrite<T, U, S>
where
    T: Sink<Bytes>,
    S: Serializer<U>,
    S::Error: Into<T::Error>,
{
    type Error = T::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: U) -> Result<(), Self::Error> {
        let res = self.as_mut().project().serializer.serialize(&item);
        let bytes = res.map_err(Into::into)?;

        self.as_mut().project().inner.start_send(bytes)?;

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_flush(cx))?;
        self.project().inner.poll_close(cx)
    }
}

#[cfg(any(feature = "json", feature = "bincode", feature = "messagepack"))]
pub mod formats {
    #[cfg(feature = "bincode")]
    pub use self::bincode::Bincode;
    #[cfg(feature = "json")]
    pub use self::json::Json;
    #[cfg(feature = "messagepack")]
    pub use self::messagepack::MessagePack;

    use {
        super::{Deserializer, Serializer},
        bytes::{buf::BufExt, Bytes, BytesMut},
        derivative::Derivative,
        serde::{Deserialize, Serialize},
        std::{io, marker::PhantomData, pin::Pin},
    };

    #[cfg(feature = "bincode")]
    mod bincode {
        use super::*;

        #[derive(Derivative)]
        #[derivative(Default(bound = ""))]
        pub struct Bincode<T> {
            ghost: PhantomData<T>,
        }

        impl<T> Deserializer<T> for Bincode<T>
        where
            T: for<'de> Deserialize<'de>,
        {
            type Error = io::Error;

            fn deserialize(self: Pin<&mut Self>, src: &BytesMut) -> Result<T, Self::Error> {
                Ok(serde_bincode::deserialize(src)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?)
            }
        }

        impl<T> Serializer<T> for Bincode<T>
        where
            T: Serialize,
        {
            type Error = io::Error;

            fn serialize(self: Pin<&mut Self>, item: &T) -> Result<Bytes, Self::Error> {
                Ok(serde_bincode::serialize(item)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    .into())
            }
        }
    }

    #[cfg(feature = "json")]
    mod json {
        use super::*;

        #[derive(Derivative)]
        #[derivative(Default(bound = ""))]
        pub struct Json<T> {
            ghost: PhantomData<T>,
        }

        impl<T> Deserializer<T> for Json<T>
        where
            for<'a> T: Deserialize<'a>,
        {
            type Error = serde_json::Error;

            fn deserialize(self: Pin<&mut Self>, src: &BytesMut) -> Result<T, Self::Error> {
                serde_json::from_reader(std::io::Cursor::new(src).reader())
            }
        }

        impl<T> Serializer<T> for Json<T>
        where
            T: Serialize,
        {
            type Error = serde_json::Error;

            fn serialize(self: Pin<&mut Self>, item: &T) -> Result<Bytes, Self::Error> {
                serde_json::to_vec(item).map(Into::into)
            }
        }
    }

    #[cfg(feature = "messagepack")]
    mod messagepack {
        use super::*;

        use std::io;

        #[derive(Derivative)]
        #[derivative(Default(bound = ""))]
        pub struct MessagePack<T> {
            ghost: PhantomData<T>,
        }

        impl<T> Deserializer<T> for MessagePack<T>
        where
            for<'a> T: Deserialize<'a>,
        {
            type Error = io::Error;

            fn deserialize(self: Pin<&mut Self>, src: &BytesMut) -> Result<T, Self::Error> {
                Ok(
                    serde_messagepack::from_read(std::io::Cursor::new(src).reader())
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
                )
            }
        }

        impl<T> Serializer<T> for MessagePack<T>
        where
            T: Serialize,
        {
            type Error = io::Error;

            fn serialize(self: Pin<&mut Self>, item: &T) -> Result<Bytes, Self::Error> {
                Ok(serde_messagepack::to_vec(item)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    .into())
            }
        }
    }
}
