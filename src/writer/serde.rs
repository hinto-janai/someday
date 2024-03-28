//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
#[cfg(any(feature = "serde", feature = "bincode", feature = "borsh",))]
use crate::{Commit, Writer};
#[allow(unused_imports)]
// docs
// use crate::Commit;

//---------------------------------------------------------------------------------------------------- Writer
#[cfg(feature = "serde")]
impl<T> serde::Serialize for Writer<T>
where
    T: Clone + serde::Serialize,
{
    #[inline]
    /// This will serialize the latest [`Commit`] of the [`Writer`].
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Commit::serialize(self.head(), serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Writer<T>
where
    T: Clone + serde::Deserialize<'de>,
{
    #[inline]
    /// This will deserialize a [`Commit`] directly into a [`Writer`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::from("hello"));
    /// w.add_commit(|w, _| {
    ///     w.push_str(" world!");
    /// });
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.data(), "hello world!");
    /// assert_eq!(r.head().timestamp, 0);
    /// assert_eq!(r.head().data, "hello");
    ///
    /// let json = serde_json::to_string(&w).unwrap();
    /// assert_eq!(json, "{\"timestamp\":1,\"data\":\"hello world!\"}");
    ///
    /// let writer: Writer<String> = serde_json::from_str(&json).unwrap();
    /// assert_eq!(writer.timestamp(), 1);
    /// assert_eq!(writer.data(), "hello world!");
    /// ```
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Commit::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(feature = "bincode")]
impl<T> bincode::Encode for Writer<T>
where
    T: Clone + bincode::Encode,
{
    #[inline]
    /// This will serialize the latest [`Commit`] of the [`Writer`].
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Commit::encode(self.head(), encoder)
    }
}

#[cfg(feature = "bincode")]
impl<T> bincode::Decode for Writer<T>
where
    T: Clone + bincode::Decode,
{
    #[inline]
    /// This will deserialize a [`Commit`] directly into a [`Writer`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::from("hello"));
    /// w.add_commit(|w, _| {
    ///     w.push_str(" world!");
    /// });
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.data(), "hello world!");
    /// assert_eq!(r.head().timestamp, 0);
    /// assert_eq!(r.head().data, "hello");
    ///
    /// let config = bincode::config::standard();
    ///
    /// // Decode into a `Commit`.
    /// let encoded = bincode::encode_to_vec(&w, config).unwrap();
    /// let decoded: Commit<String> = bincode::decode_from_slice(&encoded, config).unwrap().0;
    /// assert_eq!(decoded, Commit { timestamp: 1, data: String::from("hello world!") });
    ///
    /// // Decode directly into a `Writer<T>`.
    /// let writer: Writer<String> = bincode::decode_from_slice(&encoded, config).unwrap().0;
    /// assert_eq!(writer.timestamp(), 1);
    /// assert_eq!(writer.data(), "hello world!");
    /// ```
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Commit::decode(decoder).map(Self::from)
    }
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshSerialize for Writer<T>
where
    T: Clone + borsh::BorshSerialize,
{
    /// This will serialize the latest [`Commit`] of the [`Writer`].
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        Commit::serialize(self.head(), writer)
    }
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshDeserialize for Writer<T>
where
    T: Clone + borsh::BorshDeserialize,
{
    /// This will deserialize a [`Commit`] directly into a [`Writer`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::from("hello"));
    /// w.add_commit(|w, _| {
    ///     w.push_str(" world!");
    /// });
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.data(), "hello world!");
    /// assert_eq!(r.head().timestamp, 0);
    /// assert_eq!(r.head().data, "hello");
    ///
    /// // Decode into a `Commit`.
    /// let encoded = borsh::to_vec(&w).unwrap();
    /// let decoded: Commit<String> = borsh::from_slice(&encoded).unwrap();
    /// assert_eq!(decoded, Commit { timestamp: 1, data: String::from("hello world!") });
    ///
    /// // Decode directly into a `Writer<T>`.
    /// let writer: Writer<String> = borsh::from_slice(&encoded).unwrap();
    /// assert_eq!(writer.timestamp(), 1);
    /// assert_eq!(writer.data(), "hello world!");
    /// ```
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        Commit::deserialize_reader(reader).map(Self::from)
    }
}
