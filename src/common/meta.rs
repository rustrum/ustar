use core::ops::Range;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

use super::{BLOCK_SIZE, offset_by_blocks, pair_match_key, pair_match_value, parse_isize, parse_usize};

pub const HEADER_SIZE: usize = 500;

const ASCII_SPACE: u8 = 32;
const HEADER_MAGIC: &'static [u8; 6] = b"ustar\0";
const HEADER_VERSION: &'static [u8; 2] = b"00";

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

/// Define tar header type
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

#[derive(Debug, PartialEq)]
pub enum HeaderValidation {
    Valid,
    Invalid,
    /// Header has no data - buffer contains only zeroes
    Zeroes,
}


/// Rust friendly wrapper around POSIX header.
/// Provides friendly type safe API to access header content.
#[derive(Debug)]
pub struct Header {
    pheader: PosixHeader,
    /// Header position in source
    offset: usize,
    /// Index of previous revision (related to order in source)
    prev: Option<usize>,
}

/// Aggregate meta info about tar archive (combine all headers in easy accessible way).
#[derive(Debug)]
pub struct TarMeta {
    /// List of haders in same order as in source
    headers: Vec<Header>,

    /// Headers index by file name
    index: HashMap<String, usize>,
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


impl TarMeta {}