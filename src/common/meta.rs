// https://pubs.opengroup.org/onlinepubs/9699919799/utilities/pax.html
// https://www.gnu.org/software/tar/manual/html_node/Standard.html
// https://www.ibm.com/support/knowledgecenter/en/SSLTBW_2.1.0/com.ibm.zos.v2r1.bpxa500/taf.htm
use core::ops::Range;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

use super::{BLOCK_SIZE, offset_by_blocks, pair_match_key, pair_match_value, parse_isize, parse_usize};

pub const HEADER_SIZE: usize = 500;

const ASCII_SPACE: u8 = 32;
// Last char also could be \0
const HEADER_MAGIC: &'static [u8; 6] = b"ustar ";
const HEADER_VERSION: &'static [u8; 2] = b"00";

/// Checksum header validation status.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum HeaderCheck {
    Valid,
    /// Represents invalid header. All non ustar headers are also considered as invalid.
    /// There is corresponding property to check this.
    Invalid { not_ustar: bool },
    /// Header has no data - buffer contains only zeroes
    Zeroes,
}

/// POSIX header: tar Header Block, from POSIX 1003.1-1990.
/// This is just wrapper around raw bytes array.
pub struct PosixHeader {
    offset: usize,
    check: HeaderCheck,
    buffer: [u8; 512],
}

impl std::fmt::Debug for PosixHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PosixHeader")
    }
}

#[non_exhaustive]
pub struct Mode;

//Bits used in the mode field, values in octal.
impl Mode {
    /// set UID on execution
    pub const TSUID: u16 = 0x04000;
    /// set GID on execution
    pub const TSGID: u16 = 0x02000;
    /// reserved
    pub const TSVTX: u16 = 0x01000;
    // file permissions
    /// read by owner
    pub const TUREAD: u16 = 0x00400;
    /// write by owner
    pub const TUWRITE: u16 = 0x00200;
    /// execute/search by owner
    pub const TUEXEC: u16 = 0x00100;
    /// read by group
    pub const TGREAD: u16 = 0x00040;
    /// write by group
    pub const TGWRITE: u16 = 0x00020;
    /// execute/search by group
    pub const TGEXEC: u16 = 0x00010;
    /// read by other
    pub const TOREAD: u16 = 0x00004;
    /// write by other
    pub const TOWRITE: u16 = 0x00002;
    /// execute/search by other
    pub const TOEXEC: u16 = 0x00001;
}

/// Offsets are here: https://www.gnu.org/software/tar/manual/html_node/Standard.html
#[non_exhaustive]
pub struct HeaderProperty;

impl HeaderProperty {
    pub const Name: Range<usize> = 0..100;
    pub const Mode: Range<usize> = 100..108;
    pub const Uid: Range<usize> = 108..116;
    pub const Gid: Range<usize> = 116..124;
    pub const Size: Range<usize> = 124..136;
    pub const Mtime: Range<usize> = 136..148;
    pub const Chksum: Range<usize> = 148..156;
    pub const Typeflag: Range<usize> = 156..157;
    pub const Linkname: Range<usize> = 157..257;
    pub const Magic: Range<usize> = 257..263;
    pub const Version: Range<usize> = 263..265;
    pub const Uname: Range<usize> = 265..297;
    pub const Gname: Range<usize> = 297..329;
    pub const Devmajor: Range<usize> = 329..337;
    pub const Devminor: Range<usize> = 337..345;
    pub const Prefix: Range<usize> = 345..500;
}

/// Type of header related to typecalss property in POSIX spec.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum HeaderType {
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

const TYPE_FLAGS: [(HeaderType, u8); 11] = [
    (HeaderType::Reg, b'0'),
    (HeaderType::Link, b'1'),
    (HeaderType::Sym, b'2'),
    (HeaderType::Chr, b'3'),
    (HeaderType::Blk, b'4'),
    (HeaderType::Dir, b'5'),
    (HeaderType::Fifo, b'6'),
    (HeaderType::Cont, b'7'),
    (HeaderType::Xhd, b'x'),
    (HeaderType::Xlg, b'g'),
    // Duplicate matcher for old format
    (HeaderType::Reg, b'\0'),
];


/// Contains Rust friendly representation from POSIX header raw content.
#[derive(Debug)]
pub struct Header {
    pub check: HeaderCheck,
    /// Header position in source
    pub offset: usize,
    /// Index of previous revision (related to headers order in source)
    pub prev: Option<usize>,

    pub typeflag: HeaderType,

    pub name: String,
    // Prefix // :)
    pub linkname: String,
    pub uname: String,
    pub gname: String,
    pub mode: u64,
    // char[12]
    pub mtime: u128,
    // char[12]
    pub size: usize,
}

/// Aggregate meta info about tar archive (combine all headers in easy accessible way).
#[derive(Debug)]
pub struct TarMeta {
    /// List of haders in same order as in source
    headers: Vec<Header>,

    /// Headers index by file name
    index: HashMap<String, usize>,
}

impl Header {
    pub fn from(pheader: PosixHeader) -> Header {
        Header {
            offset: pheader.offset,
            check: pheader.check.clone(),
            prev: None,

            size: pheader.size(),
            typeflag: pheader.typeflag(),

            name: String::new(),
            linkname: String::new(),
            uname: String::new(),
            gname: String::new(),
            mode: 0,
            mtime: 0,
        }
    }
}

impl PosixHeader {
    pub fn from(offset: usize, bytes: [u8; BLOCK_SIZE]) -> PosixHeader {
        let mut ph = PosixHeader {
            offset: offset,
            buffer: bytes,
            check: HeaderCheck::Invalid { not_ustar: false },
        };
        ph.check = ph.validate();
        ph
    }

    pub fn size(&self) -> usize {
        let size_str = self.extract_string(HeaderProperty::Size);
        parse_usize(&size_str).unwrap_or_default()
    }

    pub fn typeflag(&self) -> HeaderType {
        let flag = self.extract(HeaderProperty::Typeflag)[0];
        pair_match_value(flag, &TYPE_FLAGS).unwrap_or(HeaderType::Unknown)
    }

    /// Extract property from raw buffer as it is.
    pub fn extract(&self, bytes_range: Range<usize>) -> &[u8] {
        &self.buffer[bytes_range]
    }

    pub fn extract_string(&self, bytes_range: Range<usize>) -> String {
        let v = self.extract(bytes_range);
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
    pub fn validate(&self) -> HeaderCheck {
        let mut unsigned_sum = 0_usize; // the POSIX one :-)
        let mut signed_sum = 0_isize; // the Sun one :-(
        let rchecksum = HeaderProperty::Chksum;
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
            return HeaderCheck::Zeroes;
        }

        // println!("Checksums s:{:#o} u:{:#o}", signed_sum, unsigned_sum);

        let checksum_raw = self.extract_string(HeaderProperty::Chksum);
        let checksum = parse_isize(&checksum_raw).unwrap();

        if checksum < 0 {
            return HeaderCheck::Invalid { not_ustar: false };
        }

        if unsigned_sum != checksum as usize && signed_sum != checksum {
            HeaderCheck::Invalid { not_ustar: false }
        } else {
            let magic = self.extract(HeaderProperty::Magic);
            // alternatively could check for first 5 characters
            // if magic[0..5] == HEADER_MAGIC[0..5] {
            if magic == HEADER_MAGIC {
                HeaderCheck::Valid
            } else {
                HeaderCheck::Invalid { not_ustar: true }
            }
        }
    }
}


impl TarMeta {}