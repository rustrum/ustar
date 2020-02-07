use core::clone::Clone;
use core::cmp::PartialEq;
use core::iter::Iterator;
use core::num::ParseIntError;
use core::ops::Range;
use std::io::{Read, Seek, SeekFrom};

pub const BLOCK_SIZE: usize = 512;
pub const HEADER_SIZE: usize = 500;

const ASCII_SPACE: u8 = 32;
const HEADER_MAGIC: &'static [u8; 6] = b"ustar\0";
const HEADER_VERSION: &'static [u8; 2] = b"00";

#[derive(Debug, PartialEq)]
pub enum ErrorTar {
    InvalidBlockSize,
}

/// POSIX header: tar Header Block, from POSIX 1003.1-1990.
/// This is just wrapper around raw bytes array.
pub struct PosixHeader {
    buffer: [u8; 512],
}

impl std::fmt::Debug for PosixHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PosixHeader")
    }
}

#[derive(Debug, PartialEq)]
pub enum HeaderValidation {
    Valid,
    Invalid,
    /// Header has no data - buffer contains only zeroes
    Zeroes,
}

pub enum HeaderProperty {
    Name,
    Mode,
    Uid,
    Gid,
    Size,
    Mtime,
    Chksum,
    Typeflag,
    Linkname,
    Magic,
    Version,
    Uname,
    Gname,
    Devmajor,
    Devminor,
    Prefix,
}

/// Offsets are here: https://www.gnu.org/software/tar/manual/html_node/Standard.html
fn property_range(p: HeaderProperty) -> Range<usize> {
    match p {
        HeaderProperty::Name => 0..100,
        HeaderProperty::Mode => 100..108,
        HeaderProperty::Uid => 108..116,
        HeaderProperty::Gid => 116..124,
        HeaderProperty::Size => 124..136,
        HeaderProperty::Mtime => 136..148,
        HeaderProperty::Chksum => 148..156,
        HeaderProperty::Typeflag => 156..157,
        HeaderProperty::Linkname => 157..257,
        HeaderProperty::Magic => 257..263,
        HeaderProperty::Version => 263..265,
        HeaderProperty::Uname => 265..297,
        HeaderProperty::Gname => 297..329,
        HeaderProperty::Devmajor => 329..337,
        HeaderProperty::Devminor => 337..345,
        HeaderProperty::Prefix => 345..500,
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TypeFlag {
    /// regular file
    Reg,
    /// Link
    Link,
    /// Reserver
    Sym,
    /// Character special
    Chr,
    /// Block special
    Blk,
    /// Directory
    Dir,
    /// FIFO special
    Fifo,
    /// Reserver
    Cont,
    /// Extended header referring to the next file in the archiv
    Xhd,
    /// Global extended header
    Xlg,
    Unknown,
}

const TYPE_FLAGS: [(TypeFlag, u8); 11] = [
    (TypeFlag::Reg, b'0'),
    (TypeFlag::Link, b'1'),
    (TypeFlag::Sym, b'2'),
    (TypeFlag::Chr, b'3'),
    (TypeFlag::Blk, b'4'),
    (TypeFlag::Dir, b'5'),
    (TypeFlag::Fifo, b'6'),
    (TypeFlag::Cont, b'7'),
    (TypeFlag::Xhd, b'x'),
    (TypeFlag::Xlg, b'g'),
    // Duplicate matcher for old format
    (TypeFlag::Reg, b'\0'),
];

/// Rust friendly wrapper around POSIX header.
/// Provides friendly type safe API to access header content.
#[derive(Debug)]
pub struct Header {
    pheader: PosixHeader,
    /// Header position in source
    offset: usize,
    /// Previous revision if any
    prev: Option<Box<Header>>,
}

/// Thing to extract headers from some source
#[derive(Debug)]
pub struct HeadersParser<'a, S> {
    offset: usize,
    source: &'a mut S,
}

impl PosixHeader {
    pub fn from(bytes: [u8; BLOCK_SIZE]) -> PosixHeader {
        PosixHeader { buffer: bytes }
    }

    /// Extract property from raw buffer as it is.
    pub fn extract(&self, p: HeaderProperty) -> &[u8] {
        &self.buffer[property_range(p)]
    }

    pub fn extract_string(&self, p: HeaderProperty) -> String {
        let v = self.extract(p);
        let mut range = 0..v.len();
        for i in 0..v.len() {
            if v[i] == 0 {
                range = 0..i;
                break;
            }
        }

        String::from_utf8_lossy(&v[range]).into_owned()
    }

    /// Does header checksum validation
    ///
    /// The standard BSD tar sources create the checksum by adding up the bytes in the header as type char.
    /// It looks like the sources to BSD tar were never changed to compute the checksum correctly,
    /// so both the Sun and Next add the bytes of the header as signed chars.
    /// This doesn't cause a problem until you get a file with a name containing characters with the high bit set.
    /// So tar_checksum computes two checksums -- signed and unsigned.
    pub fn validate(&self) -> HeaderValidation {
        let mut unsigned_sum = 0_usize; // the POSIX one :-)
        let mut signed_sum = 0_isize; // the Sun one :-(
        let rchecksum = property_range(HeaderProperty::Chksum);
        let mut zeroes = true;

        for i in 0..HEADER_SIZE {
            let mut value = self.buffer[i];
            if value != 0 {
                zeroes = false;
            }
            if rchecksum.contains(&i) {
                value = ASCII_SPACE;
            }
            unsigned_sum += value as usize;
            signed_sum += (value as i8) as isize;
        }

        if zeroes {
            return HeaderValidation::Zeroes;
        }

        // println!("Checksums s:{:#o} u:{:#o}", signed_sum, unsigned_sum);

        let checksum_raw = self.extract_string(HeaderProperty::Chksum);
        let checksum = parse_isize(&checksum_raw).unwrap();

        if checksum < 0 {
            return HeaderValidation::Invalid;
        }

        if unsigned_sum != checksum as usize && signed_sum != checksum {
            HeaderValidation::Invalid
        } else {
            HeaderValidation::Valid
        }
    }
}

impl Header {
    pub fn from(offset: usize, bytes: [u8; BLOCK_SIZE]) -> Header {
        Header {
            offset: offset,
            pheader: PosixHeader { buffer: bytes },
            prev: None,
        }
    }

    pub fn source(&self) -> &PosixHeader {
        &self.pheader
    }

    pub fn size(&self) -> usize {
        let size_str = self.pheader.extract_string(HeaderProperty::Size);
        parse_usize(&size_str).unwrap_or_default()
    }

    pub fn typeflag(&self) -> TypeFlag {
        let flag = self.pheader.extract(HeaderProperty::Typeflag)[0];
        pair_match_value(flag, &TYPE_FLAGS).unwrap_or(TypeFlag::Unknown)
    }
}

impl<'a, T: Read + Seek> HeadersParser<'a, T> {
    fn from(reader: &'a mut T) -> HeadersParser<'a, T> {
        reader.seek(SeekFrom::Start(0));
        HeadersParser {
            offset: 0,
            source: reader,
        }
    }

    /// Read any bytes as block
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
        Some(h)
    }
}

impl<'a, T: Read + Seek> Iterator for HeadersParser<'a, T> {
    type Item = Header;

    /// Iterate only over valid blocks
    fn next(&mut self) -> Option<Self::Item> {
        let h = self.next_any()?;

        if let HeaderValidation::Valid = h.source().validate() {
            Some(h)
        } else {
            None
        }
    }
}

fn offset_by_blocks(bytes_count: usize) -> usize {
    let offset = bytes_count / BLOCK_SIZE * BLOCK_SIZE;
    if bytes_count % BLOCK_SIZE == 0 {
        offset
    } else {
        offset + BLOCK_SIZE
    }
}

fn parse_usize(string: &str) -> Result<usize, ParseIntError> {
    let strval = &string.trim_end_matches(char::from(0));

    // println!("usize parsed from {}", strval);
    usize::from_str_radix(strval, 8)
}

fn parse_isize(string: &str) -> Result<isize, ParseIntError> {
    let strval = &string.trim_matches(char::from(0));

    // println!("Isize parsed from {} {:?}", strval, strval.as_bytes());
    isize::from_str_radix(strval, 8)
}

fn pair_match_value<K: Clone, V: PartialEq>(value: V, pairs: &[(K, V)]) -> Option<K> {
    for i in 0..pairs.len() {
        let p = &pairs[i];
        if p.1 == value {
            return Some(p.0.clone());
        }
    }
    None
}

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

    use super::*;
    use hamcrest2::prelude::*;
    use std::fs::File;
    use std::path::{Path, PathBuf};

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
