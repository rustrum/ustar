pub mod common;

// pub use common::{ErrorTar, HeaderProperty, HeaderValidation, PosixHeader, BLOCK_SIZE};

use core::iter::Iterator;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub enum TarError {
    ReadData,
}
