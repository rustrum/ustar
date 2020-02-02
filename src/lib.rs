use core::iter::Iterator;
use core::num::ParseIntError;
use std::io::{Read, Seek, SeekFrom};

const BLOCK_SIZE: usize = 512;

/// POSIX header: tar Header Block, from POSIX 1003.1-1990.
// #[derive(Debug)]
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct PosixHeader {
    /// Offset: 0  
    name: [u8; 100],
    /// Offset: 100
    mode: [u8; 8],
    /// Offset: 108
    uid: [u8; 8],
    /// Offset: 116
    gid: [u8; 8],
    /// Offset: 124
    size: [u8; 12],
    /// Offset: 136
    mtime: [u8; 12],
    /// Offset: 148
    chksum: [u8; 8],
    /// Offset: 156
    typeflag: u8,
    /// Offset: 157
    linkname: [u8; 100],
    /// Offset: 257
    magic: [u8; 6],
    /// Offset: 263
    version: [u8; 2],
    /// Offset: 265
    uname: [u8; 32],
    /// Offset: 297
    gname: [u8; 32],
    /// Offset: 329
    devmajor: [u8; 8],
    /// Offset: 337
    devminor: [u8; 8],
    /// Offset: 345 - 500
    prefix: [u8; 155],
}

#[derive(Debug)]
pub enum TarError {
    ReadData,
}
// use std::mem;

fn parse_usize(string: &str) -> Result<usize, ParseIntError> {
    println!("Parsing {:?}", string.as_bytes());
    usize::from_str_radix(&string, 8)
}

fn zeroes(slice: &[u8]) -> bool {
    for i in 0..slice.len() {
        if slice[i] != 0 {
            return false;
        }
    }
    true
}

impl PosixHeader {
    fn from(bytes: &[u8]) -> Result<PosixHeader, TarError> {
        if bytes.len() < BLOCK_SIZE {
            return Err(TarError::ReadData);
        }

        // println!("HEADER size {}", mem::size_of::<PosixHeader>());
        let (head, body, _tail) = unsafe { bytes.align_to::<PosixHeader>() };

        // println!("Readed {}, {}, {}", head.len(), body.len(), _tail.len());
        if !head.is_empty() || body.is_empty() {
            return Err(TarError::ReadData);
        }
        Ok(body[0])
    }

    fn size(&self) -> usize {
        let size_str = String::from_utf8_lossy(&self.size);
        parse_usize(&size_str.trim_end_matches(char::from(0))).unwrap()
    }
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

        match PosixHeader::from(&buffer) {
            Ok(h) => {
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
            Err(e) => {
                println!(
                    "Can not read PosixHeader from offset {}: {:?}",
                    self.offset - BLOCK_SIZE,
                    e
                );
                None
            }
        }
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

        let file_1 = headers[0];
        assert_that!(file_1.size(), equal_to(512));

        let file_2 = headers[1];
        assert_that!(file_2.size(), less_than(512));

        let file_3 = headers[2];
        assert_that!(file_3.size(), greater_than(512));

        let file_4 = headers[3];
        assert_that!(file_4.size(), less_than(512));
    }

    #[test]
    fn headers_reading_append() {
        let path = test_resources_path().join("files_append_test.tar");
        let mut file = File::open(&path).unwrap();

        let hi = HeadersIterator::from(&mut file);

        let headers = hi.collect::<Vec<PosixHeader>>();

        assert_eq!(headers.len(), 3);

        let file_1 = headers[0];
        assert_that!(file_1.size(), greater_than(0));
        let mut prev_size = file_1.size();

        let file_2 = headers[1];
        assert_that!(file_2.size(), greater_than(prev_size));
        prev_size = file_2.size();

        let file_3 = headers[2];
        assert_that!(file_3.size(), greater_than(prev_size));
    }
}
