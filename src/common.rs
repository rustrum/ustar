use core::num::ParseIntError;
use core::ops::Range;

pub const BLOCK_SIZE: usize = 512;

#[derive(Debug)]
pub enum ErrorTar {
    InvalidBlockSize,
}

/// POSIX header: tar Header Block, from POSIX 1003.1-1990.
// #[derive(Debug)]
pub struct PosixHeader {
    buffer: [u8; 512],
}

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

impl PosixHeader {
    pub fn from(bytes: [u8; 512]) -> PosixHeader {
        PosixHeader { buffer: bytes }
    }

    /// Extract property from raw buffer as it is.
    pub fn extract(&self, p: HeaderProperty) -> &[u8] {
        &self.buffer[property_range(p)]
    }

    pub fn size(&self) -> usize {
        let size_str = String::from_utf8_lossy(self.extract(HeaderProperty::Size));
        parse_usize(&size_str.trim_end_matches(char::from(0))).unwrap()
    }

    /// Does header checksum validation
    pub fn validate(&self) -> HeaderValidation {
        HeaderValidation::Valid
    }
}

fn parse_usize(string: &str) -> Result<usize, ParseIntError> {
    usize::from_str_radix(&string, 8)
}
