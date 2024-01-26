//! `Writer<T>` and it's associated methods spilt up.

mod writer;
pub use writer::Writer;

mod add_commit_push;
mod get;
mod push;
mod pull;
mod fork;
mod timestamp;
mod tag;
mod misc;
mod serde;