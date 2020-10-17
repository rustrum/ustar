pub mod meta;

use self::meta::BLOCK_SIZE;
use self::meta::Header;
use self::meta::HeaderValidation;
use self::meta::PosixHeader;
use self::meta::TypeFlag;

use core::clone::Clone;
use core::cmp::PartialEq;
use core::iter::Iterator;
use core::num::ParseIntError;
use core::ops::Range;
use std::io::{Read, Seek, SeekFrom};


#[derive(Debug, PartialEq)]
pub enum ErrorTar {
    InvalidBlockSize,
}

/// Extracts tar Headers from some source.
#[derive(Debug)]
pub struct HeadersParser<'a, S> {
    offset: usize,
    source: &'a mut S,
    iter_valid_headers: usize,
    iter_invalid_headers: usize,
    iter_zeroes: u8,
}


impl<'a, T: Read + Seek> HeadersParser<'a, T> {
    fn from(reader: &'a mut T) -> HeadersParser<'a, T> {
        reader.seek(SeekFrom::Start(0));
        HeadersParser {
            offset: 0,
            source: reader,
            iter_valid_headers: 0,
            iter_invalid_headers: 0,
            iter_zeroes: 0,
        }
    }

    /// Read any bytes as block.
    /// It is possible that we could have invalid header somewhere in the middle but with proper size attribute,
    /// thus it would be possible to shift to the next valid header.
    fn next_any(&mut self) -> Option<Header> {
        let mut buffer = [0; BLOCK_SIZE];
        // Assuming it would shift position at number of buffer
        self.source.read_exact(&mut buffer).ok()?;
        self.offset += BLOCK_SIZE;

        // print!("BUFFER: ");
        // for i in 0..BLOCK_SIZE {
        //     print!("{}", buffer[i]);
        // }
        // println!("");

        let h = Header::from(self.offset, buffer);
        let size = h.size();
        let shift = offset_by_blocks(size);

        //println!("File size {} shift {}", size, shift);

        self.offset += shift;
        self.source.seek(SeekFrom::Current(shift as i64));

        // Now lets collect some stats
        match &h.source().validate() {
            HeaderValidation::Valid => {
                self.iter_valid_headers += 1;
                if self.iter_zeroes > 0 {
                    // Valid header could not be after zero header - consider this as an error.
                    self.iter_invalid_headers += 1;
                }
            }
            HeaderValidation::Invalid => {
                self.iter_invalid_headers += 1;
            }
            HeaderValidation::Zeroes => {
                if self.iter_zeroes > 2 {
                    // Only 2 zero headers allowed
                    self.iter_invalid_headers += 1;
                }
                self.iter_zeroes += 1;
            }
        }
        Some(h)
    }
}

impl<'a, T: Read + Seek> Iterator for HeadersParser<'a, T> {
    type Item = Header;

    /// Iterate only over valid blocks.
    /// Last two blocks are just zeroes so we just ignore them (not valid).
    fn next(&mut self) -> Option<Self::Item> {
        let h = self.next_any()?;

        if let HeaderValidation::Valid = h.source().validate() {
            Some(h)
        } else {
            None
        }
    }
}


/// Giver bytes count return offset that divisible by blocks size.
fn offset_by_blocks(bytes_count: usize) -> usize {
    let offset = bytes_count / BLOCK_SIZE * BLOCK_SIZE;
    if bytes_count % BLOCK_SIZE == 0 {
        offset
    } else {
        offset + BLOCK_SIZE
    }
}

/// Just read usize from string
fn parse_usize(string: &str) -> Result<usize, ParseIntError> {
    let strval = &string.trim_end_matches(char::from(0));
    // println!("usize parsed from {}", strval);
    usize::from_str_radix(strval, 8)
}

/// Just read isize from string
fn parse_isize(string: &str) -> Result<isize, ParseIntError> {
    let strval = &string.trim_matches(char::from(0));
    // println!("Isize parsed from {} {:?}", strval, strval.as_bytes());
    isize::from_str_radix(strval, 8)
}

/// Return key from slice of pairs (K,V) by value.
fn pair_match_value<K: Clone, V: PartialEq>(value: V, pairs: &[(K, V)]) -> Option<K> {
    for i in 0..pairs.len() {
        let p = &pairs[i];
        if p.1 == value {
            return Some(p.0.clone());
        }
    }
    None
}

/// Return value from slice of pairs (K,V) by key.
fn pair_match_key<K: PartialEq, V: Clone>(key: K, pairs: &[(K, V)]) -> Option<V> {
    for i in 0..pairs.len() {
        let p = &pairs[i];
        if p.0 == key {
            return Some(p.1.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;
    use std::path::{Path, PathBuf};

    use hamcrest2::prelude::*;

    use super::*;

    #[test]
    fn zero_header_validation() {
        let zeroes = [0; BLOCK_SIZE];
        let header = PosixHeader::from(zeroes);

        assert_that!(header.validate(), equal_to(HeaderValidation::Zeroes));
    }

    fn test_resources_path() -> PathBuf {
        let basedir = env::var("CARGO_MANIFEST_DIR").unwrap();
        Path::new(&basedir).join("test")
    }

    fn basic_header_validation(h: &Header) {
        assert_that!(h.source().validate(), equal_to(HeaderValidation::Valid));
        assert_that!(h.typeflag(), not(equal_to(TypeFlag::Unknown)));

        // assert_that!(
        //     &h.source().extract(HeaderProperty::Magic).to_vec(),
        //     contains(HEADER_MAGIC.to_vec())
        // );

        // assert_that!(
        //     &h.source().extract(HeaderProperty::Version).to_vec(),
        //     contains(HEADER_VERSION.to_vec())
        // );
    }

    #[test]
    fn headers_reading() {
        let path = test_resources_path().join("files_test.tar");
        let mut file = File::open(&path).unwrap();

        let hi = HeadersParser::from(&mut file);

        let headers = hi.collect::<Vec<Header>>();

        assert_eq!(headers.len(), 4);

        let file_1 = &headers[0];
        basic_header_validation(&file_1);
        assert_that!(file_1.size(), equal_to(512));

        let file_2 = &headers[1];
        basic_header_validation(&file_2);
        assert_that!(file_2.size(), less_than(512));

        let file_3 = &headers[2];
        basic_header_validation(&file_3);
        assert_that!(file_3.size(), greater_than(512));
        let file_4 = &headers[3];
        basic_header_validation(&file_4);
        assert_that!(file_4.size(), less_than(512));
    }

    #[test]
    fn headers_reading_append() {
        let path = test_resources_path().join("files_append_test.tar");
        let mut file = File::open(&path).unwrap();

        let hi = HeadersParser::from(&mut file);

        let headers = hi.collect::<Vec<Header>>();

        assert_eq!(headers.len(), 3);

        let file_1 = &headers[0];
        basic_header_validation(&file_1);
        assert_that!(file_1.size(), greater_than(0));
        let mut prev_size = file_1.size();

        let file_2 = &headers[1];
        basic_header_validation(&file_2);
        assert_that!(file_2.size(), greater_than(prev_size));
        prev_size = file_2.size();

        let file_3 = &headers[2];
        basic_header_validation(&file_3);
        assert_that!(file_3.size(), greater_than(prev_size));
    }
}
