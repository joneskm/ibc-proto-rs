//! This module provides an object safe equivalent of the `tendermint_proto::Protobuf` trait,
//! thereby allowing for easy Google Protocol Buffers encoding and decoding of domain types with
//! validation.
//!
//! Domain types implementing the `Protobuf` trait are expected to implement `TryFrom<T>` and
//! `Into<T>` where `T` is the raw type. The equivalent object safe `erased` counterparts have
//! blanket implementations and are derived automatically.
//!
//! ## Examples
//!
//! ```rust
//! use core::convert::TryFrom;
//!
//! use prost::Message;
//! use ibc_proto::protobuf::Protobuf;
//!
//! // This struct would ordinarily be automatically generated by prost.
//! #[derive(Clone, PartialEq, Message)]
//! pub struct MyRawType {
//!     #[prost(uint64, tag="1")]
//!     pub a: u64,
//!     #[prost(string, tag="2")]
//!     pub b: String,
//! }
//!
//! #[derive(Clone)]
//! pub struct MyDomainType {
//!     a: u64,
//!     b: String,
//! }
//!
//! impl MyDomainType {
//!     // Trivial constructor with basic validation logic.
//!     pub fn new(a: u64, b: String) -> Result<Self, String> {
//!         if a < 1 {
//!             return Err("a must be greater than 0".to_owned());
//!         }
//!         Ok(Self { a, b })
//!     }
//! }
//!
//! impl TryFrom<MyRawType> for MyDomainType {
//!     type Error = String;
//!
//!     fn try_from(value: MyRawType) -> Result<Self, Self::Error> {
//!         Self::new(value.a, value.b)
//!     }
//! }
//!
//! impl From<MyDomainType> for MyRawType {
//!     fn from(value: MyDomainType) -> Self {
//!         Self { a: value.a, b: value.b }
//!     }
//! }
//!
//! impl Protobuf<MyRawType> for MyDomainType {}
//! ```
pub mod erased;
mod error;

#[allow(unused_imports)]
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Display;

use bytes::Buf;
use prost::Message;
use subtle_encoding::hex;

pub use self::error::Error;

/// Object safe equivalent of `tendermint_proto::Protobuf`.
pub trait Protobuf<Raw: Message + Default>
where
    Self: erased::TryFrom<Raw> + erased::CloneInto<Raw>,
    <Self as erased::TryFrom<Raw>>::Error: Display,
{
    /// Encode into a buffer in Protobuf format.
    ///
    /// Uses [`prost::Message::encode`] after converting into its counterpart
    /// Protobuf data structure.
    ///
    /// [`prost::Message::encode`]: https://docs.rs/prost/*/prost/trait.Message.html#method.encode
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        self.clone_into().encode(buf).map_err(Error::encode_message)
    }

    /// Encode with a length-delimiter to a buffer in Protobuf format.
    ///
    /// An error will be returned if the buffer does not have sufficient capacity.
    ///
    /// Uses [`prost::Message::encode_length_delimited`] after converting into
    /// its counterpart Protobuf data structure.
    ///
    /// [`prost::Message::encode_length_delimited`]: https://docs.rs/prost/*/prost/trait.Message.html#method.encode_length_delimited
    fn encode_length_delimited(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        self.clone_into()
            .encode_length_delimited(buf)
            .map_err(Error::encode_message)
    }

    /// Constructor that attempts to decode an instance from a buffer.
    ///
    /// The entire buffer will be consumed.
    ///
    /// Similar to [`prost::Message::decode`] but with additional validation
    /// prior to constructing the destination type.
    ///
    /// [`prost::Message::decode`]: https://docs.rs/prost/*/prost/trait.Message.html#method.decode
    fn decode<B: Buf>(buf: B) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let raw = Raw::decode(buf).map_err(Error::decode_message)?;

        Self::try_from(raw).map_err(Error::try_from::<Raw, Self, _>)
    }

    /// Constructor that attempts to decode a length-delimited instance from
    /// the buffer.
    ///
    /// The entire buffer will be consumed.
    ///
    /// Similar to [`prost::Message::decode_length_delimited`] but with
    /// additional validation prior to constructing the destination type.
    ///
    /// [`prost::Message::decode_length_delimited`]: https://docs.rs/prost/*/prost/trait.Message.html#method.decode_length_delimited
    fn decode_length_delimited<B: Buf>(buf: B) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let raw = Raw::decode_length_delimited(buf).map_err(Error::decode_message)?;

        Self::try_from(raw).map_err(Error::try_from::<Raw, Self, _>)
    }

    /// Returns the encoded length of the message without a length delimiter.
    ///
    /// Uses [`prost::Message::encoded_len`] after converting to its
    /// counterpart Protobuf data structure.
    ///
    /// [`prost::Message::encoded_len`]: https://docs.rs/prost/*/prost/trait.Message.html#method.encoded_len
    fn encoded_len(&self) -> usize {
        self.clone_into().encoded_len()
    }

    /// Encodes into a Protobuf-encoded `Vec<u8>`.
    fn encode_vec(&self) -> Vec<u8> {
        self.clone_into().encode_to_vec()
    }

    /// Constructor that attempts to decode a Protobuf-encoded instance from a
    /// `Vec<u8>` (or equivalent).
    fn decode_vec(v: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Self::decode(v)
    }

    /// Encode with a length-delimiter to a `Vec<u8>` Protobuf-encoded message.
    fn encode_length_delimited_vec(&self) -> Vec<u8> {
        self.clone_into().encode_length_delimited_to_vec()
    }

    /// Constructor that attempts to decode a Protobuf-encoded instance with a
    /// length-delimiter from a `Vec<u8>` or equivalent.
    fn decode_length_delimited_vec(v: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Self::decode_length_delimited(v)
    }

    fn encode_to_hex_string(&self) -> String {
        let buf = self.encode_vec();
        let encoded = hex::encode(buf);
        String::from_utf8(encoded).expect("hex-encoded string should always be valid UTF-8")
    }
}

#[cfg(test)]
mod test {
    use core::convert::{From, TryFrom};

    use super::*;
    use crate::google::protobuf::Any;

    #[test]
    fn test_protobuf_object_safety() {
        let _test: Option<Box<dyn Protobuf<Any, Error = Error>>> = None;
    }

    #[test]
    fn test_protobuf_blanket_impls() {
        trait Foo: Protobuf<Any, Error = Error> {}

        #[derive(Clone)]
        struct Domain;

        impl Foo for Domain {}

        impl Protobuf<Any> for Domain {}

        impl TryFrom<Any> for Domain {
            type Error = Error;

            fn try_from(_: Any) -> Result<Self, Self::Error> {
                unimplemented!()
            }
        }

        impl From<Domain> for Any {
            fn from(_: Domain) -> Self {
                unimplemented!()
            }
        }
    }
}
