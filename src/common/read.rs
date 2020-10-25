use std::io::{Read, Seek, SeekFrom};

use crate::common::meta::PosixHeader;

use super::BLOCK_SIZE;
use super::meta::{Header, HeaderCheck};
use super::offset_by_blocks;

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

        let ph = PosixHeader::from(self.offset, buffer);
        ///TODO Should change approach and check validation first

        let h = Header::from(ph);
        let size = h.size;
        let shift = offset_by_blocks(size);

        //println!("File size {} shift {}", size, shift);

        self.offset += shift;
        self.source.seek(SeekFrom::Current(shift as i64));

        // Now lets collect some stats
        match &h.check {
            HeaderCheck::Valid => {
                self.iter_valid_headers += 1;
                if self.iter_zeroes > 0 {
                    // Valid header could not be after zero header - consider this as an error.
                    self.iter_invalid_headers += 1;
                }
            }
            HeaderCheck::Invalid { not_ustar } => {
                self.iter_invalid_headers += 1;
            }
            HeaderCheck::Zeroes => {
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

        if let HeaderCheck::Valid = h.check {
            Some(h)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::{Path, PathBuf};

    use hamcrest2::prelude::*;

    use super::*;
    use super::super::meta::*;

    #[test]
    fn zero_header_validation() {
        let zeroes = [0; BLOCK_SIZE];
        let header = PosixHeader::from(0, zeroes);

        assert_that!(header.validate(), equal_to(HeaderCheck::Zeroes));
    }

    fn test_resources_path() -> PathBuf {
        let basedir = env::var("CARGO_MANIFEST_DIR").unwrap();
        Path::new(&basedir).join("test")
    }

    fn basic_header_validation(h: &Header) {
        assert_that!(h.check, equal_to(HeaderCheck::Valid));
        assert_that!(h.typeflag, not(equal_to(HeaderType::Unknown)));

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
        assert_that!(file_1.size, equal_to(512));

        let file_2 = &headers[1];
        basic_header_validation(&file_2);
        assert_that!(file_2.size, less_than(512));

        let file_3 = &headers[2];
        basic_header_validation(&file_3);
        assert_that!(file_3.size, greater_than(512));
        let file_4 = &headers[3];
        basic_header_validation(&file_4);
        assert_that!(file_4.size, less_than(512));
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
        assert_that!(file_1.size, greater_than(0));
        let mut prev_size = file_1.size;

        let file_2 = &headers[1];
        basic_header_validation(&file_2);
        assert_that!(file_2.size, greater_than(prev_size));
        prev_size = file_2.size;

        let file_3 = &headers[2];
        basic_header_validation(&file_3);
        assert_that!(file_3.size, greater_than(prev_size));
    }
}
