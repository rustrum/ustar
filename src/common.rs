use core::num::ParseIntError;
use core::ops::Range;

pub const BLOCK_SIZE: usize = 512;
pub const HEADER_SIZE: usize = 500;
const ASCII_SPACE: u8 = 32;

#[derive(Debug, PartialEq)]
pub enum ErrorTar {
    InvalidBlockSize,
}

/// POSIX header: tar Header Block, from POSIX 1003.1-1990.
// #[derive(Debug)]
pub struct PosixHeader {
    buffer: [u8; 512],
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

impl PosixHeader {
    pub fn from(bytes: [u8; 512]) -> PosixHeader {
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

    pub fn size(&self) -> usize {
        let size_str = self.extract_string(HeaderProperty::Size);
        parse_usize(&size_str).unwrap()
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

        println!("Checksums s:{:#o} u:{:#o}", signed_sum, unsigned_sum);

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

#[cfg(test)]
mod tests {
    use super::*;
    use hamcrest2::prelude::*;

    #[test]
    fn zero_header_validation() {
        let zeroes = [0; BLOCK_SIZE];
        let header = PosixHeader::from(zeroes);

        assert_that!(header.validate(), equal_to(HeaderValidation::Zeroes));
    }
}
