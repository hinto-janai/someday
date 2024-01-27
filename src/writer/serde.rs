//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{
	sync::{Arc,
		atomic::{
			AtomicBool,
			Ordering,
		},
	},
	time::Duration,
	borrow::Borrow,
	collections::BTreeMap,
	num::NonZeroUsize,
};

use crate::{
	writer::Writer,
	patch::Patch,
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,WriterInfo,
	},
};

/// TODO:
/// - preserve timestamps
/// - error on incorrect data (tag timestamp > local.timestamp)

//---------------------------------------------------------------------------------------------------- Writer
#[cfg(feature = "serde")]
impl<T> serde::Serialize for Writer<T>
where
	T: Clone + serde::Serialize
{
	#[inline]
	/// This will call `data()`, then serialize your `T`.
	///
	/// `T::serialize(self.data(), serializer)`
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let json = serde_json::to_string(&w).unwrap();
	/// assert_eq!(json, "\"hello\"");
	/// ```
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
		T::serialize(self.data(), serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Writer<T>
where
	T: Clone + serde::Deserialize<'de>
{
	#[inline]
	/// This will deserialize your data `T` directly into a `Writer`.
	///
	/// `T::deserialize(deserializer).map(|t| crate::new(t).1)`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let json = serde_json::to_string(&w).unwrap();
	/// assert_eq!(json, "\"hello\"");
	///
	/// let writer: Writer<String> = serde_json::from_str(&json).unwrap();
	/// assert_eq!(writer.data(), "hello");
	/// ```
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>
	{
		T::deserialize(deserializer).map(Self::from)
	}
}

#[cfg(feature = "bincode")]
impl<T> bincode::Encode for Writer<T>
where
	T: Clone + bincode::Encode
{
	#[inline]
	/// This will call `data()`, then serialize your `T`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	/// let config = bincode::config::standard();
	///
	/// let bytes = bincode::encode_to_vec(&w, config).unwrap();
	/// assert_eq!(bytes, bincode::encode_to_vec(&"hello", config).unwrap());
	/// ```
	fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
		T::encode(self.data(), encoder)
	}
}

#[cfg(feature = "bincode")]
impl<T> bincode::Decode for Writer<T>
where
	T: Clone + bincode::Decode
{
	#[inline]
	/// This will deserialize your data `T` directly into a `Writer`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	/// let config = bincode::config::standard();
	///
	/// let bytes = bincode::encode_to_vec(&w, config).unwrap();
	/// assert_eq!(bytes, bincode::encode_to_vec(&"hello", config).unwrap());
	///
	/// let writer: Writer<String> = bincode::decode_from_slice(&bytes, config).unwrap().0;
	/// assert_eq!(writer.data(), "hello");
	/// ```
	fn decode<D: bincode::de::Decoder>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
		T::decode(decoder).map(Self::from)
	}
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshSerialize for Writer<T>
where
	T: Clone + borsh::BorshSerialize
{
	/// This will call `data()`, then serialize your `T`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let bytes = borsh::to_vec(&w).unwrap();
	/// assert_eq!(bytes, borsh::to_vec(&"hello").unwrap());
	/// ```
	fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
		T::serialize(self.data(), writer)
	}
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshDeserialize for Writer<T>
where
	T: Clone + borsh::BorshDeserialize
{
	/// This will deserialize your data `T` directly into a `Writer`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let bytes = borsh::to_vec(&w).unwrap();
	/// assert_eq!(bytes, borsh::to_vec(&"hello").unwrap());
	///
	/// let writer: Writer<String> = borsh::from_slice(&bytes).unwrap();
	/// assert_eq!(writer.data(), "hello");
	/// ```
	fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
		T::deserialize_reader(reader).map(Self::from)
	}
}