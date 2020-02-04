mod common;

pub use common::{ErrorTar, HeaderProperty, PosixHeader, BLOCK_SIZE};

use core::iter::Iterator;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub enum TarError {
    ReadData,
}
// use std::mem;

fn zeroes(slice: &[u8]) -> bool {
    for i in 0..slice.len() {
        if slice[i] != 0 {
            return false;
        }
    }
    true
}

pub struct HeadersIterator<'a, S> {
    offset: usize,
    source: &'a mut S,
}

impl<'a, T: Read + Seek> HeadersIterator<'a, T> {
    fn from(reader: &'a mut T) -> HeadersIterator<'a, T> {
        reader.seek(SeekFrom::Start(0));
        HeadersIterator {
            offset: 0,
            source: reader,
        }
    }
}

impl<'a, T: Read + Seek> Iterator for HeadersIterator<'a, T> {
    type Item = PosixHeader;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buffer = [0; BLOCK_SIZE];

        println!("readin from {}", self.offset);
        self.source.read_exact(&mut buffer).ok()?;
        if zeroes(&buffer) {
            return None;
        }
        self.offset += BLOCK_SIZE;

        print!("BUFFER: ");
        for i in 0..BLOCK_SIZE {
            print!("{}", buffer[i]);
        }
        println!("");
        // println!("Buffer {:?}", buffer);
        let h = PosixHeader::from(buffer);
        let size = h.size();
        let shift = (size / BLOCK_SIZE * BLOCK_SIZE)
            + if size % BLOCK_SIZE == 0 {
                0
            } else {
                BLOCK_SIZE
            };
        println!("File size {} shift {}", size, shift);

        self.offset += shift;
        self.source.seek(SeekFrom::Current(shift as i64));
        Some(h)
    }
}

#[cfg(test)]
mod tests {
    // todo!("Check file with exact size of 512 bytes");
    // todo!("Check file with exact size of 512 bytes");
    use std::env;

    use super::*;
    use hamcrest2::prelude::*;
    use std::fs::File;
    use std::path::{Path, PathBuf};

    fn test_resources_path() -> PathBuf {
        let basedir = env::var("CARGO_MANIFEST_DIR").unwrap();
        Path::new(&basedir).join("test")
    }

    #[test]
    fn headers_reading() {
        let path = test_resources_path().join("files_test.tar");
        let mut file = File::open(&path).unwrap();

        let hi = HeadersIterator::from(&mut file);

        let headers = hi.collect::<Vec<PosixHeader>>();

        assert_eq!(headers.len(), 4);

        let file_1 = &headers[0];
        assert_that!(file_1.size(), equal_to(512));

        let file_2 = &headers[1];
        assert_that!(file_2.size(), less_than(512));

        let file_3 = &headers[2];
        assert_that!(file_3.size(), greater_than(512));

        let file_4 = &headers[3];
        assert_that!(file_4.size(), less_than(512));
    }

    #[test]
    fn headers_reading_append() {
        let path = test_resources_path().join("files_append_test.tar");
        let mut file = File::open(&path).unwrap();

        let hi = HeadersIterator::from(&mut file);

        let headers = hi.collect::<Vec<PosixHeader>>();

        assert_eq!(headers.len(), 3);

        let file_1 = &headers[0];
        assert_that!(file_1.size(), greater_than(0));
        let mut prev_size = file_1.size();

        let file_2 = &headers[1];
        assert_that!(file_2.size(), greater_than(prev_size));
        prev_size = file_2.size();

        let file_3 = &headers[2];
        assert_that!(file_3.size(), greater_than(prev_size));
    }
}
