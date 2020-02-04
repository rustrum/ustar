mod common;

pub use common::{ErrorTar, HeaderProperty, HeaderValidation, PosixHeader, BLOCK_SIZE};

use core::iter::Iterator;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub enum TarError {
    ReadData,
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

        // println!("readin from {}", self.offset);
        self.source.read_exact(&mut buffer).ok()?;

        // print!("BUFFER: ");
        // for i in 0..BLOCK_SIZE {
        //     print!("{}", buffer[i]);
        // }
        // println!("");

        let h = PosixHeader::from(buffer);
        if let HeaderValidation::Zeroes = h.validate() {
            return None;
        }
        self.offset += BLOCK_SIZE;

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
        assert_that!(file_1.validate(), equal_to(HeaderValidation::Valid));

        let file_2 = &headers[1];
        assert_that!(file_2.size(), less_than(512));
        assert_that!(file_2.validate(), equal_to(HeaderValidation::Valid));

        let file_3 = &headers[2];
        assert_that!(file_3.size(), greater_than(512));
        assert_that!(file_3.validate(), equal_to(HeaderValidation::Valid));

        let file_4 = &headers[3];
        assert_that!(file_4.size(), less_than(512));
        assert_that!(file_4.validate(), equal_to(HeaderValidation::Valid));
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
        assert_that!(file_1.validate(), equal_to(HeaderValidation::Valid));
        let mut prev_size = file_1.size();

        let file_2 = &headers[1];
        assert_that!(file_2.size(), greater_than(prev_size));
        assert_that!(file_2.validate(), equal_to(HeaderValidation::Valid));
        prev_size = file_2.size();

        let file_3 = &headers[2];
        assert_that!(file_3.validate(), equal_to(HeaderValidation::Valid));
        assert_that!(file_3.size(), greater_than(prev_size));
    }
}
