use tokio::io::{AsyncRead, AsyncSeek};
use tokio::time::Instant;

use super::states::JobState;
use super::tube::Pri;

#[derive(Debug)]
pub struct Job {
    pub pri: Pri,
    pub data: Vec<u8>,
    pub state: JobState, // also contains state-specific data
    pub created: Instant,
    pub ttr: u32,
    pub reserves: u64,
    pub timeouts: u64,
    pub releases: u64,
    pub buries: u64,
    pub kicks: u64,
}

/// AsyncReadSeek is a supertrait, implemented automatically for all types that
/// implement AsyncRead and AsyncSeek, that represents a repeatedly readable
/// sequence of bytes stored somewhere.
///
/// The most useful implementations of this trait are [Cursor](std::io::Cursor)
/// and [File](tokio::fs::File).
///
/// ```
/// use beanstalk_rs::types::job::AsyncReadSeek;
/// use std::io::Cursor;
/// use tokio::fs::File;
/// use tokio_test::block_on;
///
/// block_on(async {
///     let from_memory: Box<dyn AsyncReadSeek> = Box::new(Cursor::new(&[1, 2, 3]));
///     let from_file: Box<dyn AsyncReadSeek> = Box::new(File::open("README.md").await.unwrap());
/// });
/// ```
pub trait AsyncReadSeek: AsyncRead + AsyncSeek {}
impl<T: AsyncRead + AsyncSeek> AsyncReadSeek for T {}
