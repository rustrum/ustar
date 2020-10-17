use core::clone::Clone;
use core::cmp::PartialEq;
use core::iter::Iterator;
use core::num::ParseIntError;
use core::ops::Range;
use std::io::{Read, Seek, SeekFrom};

use self::meta::Header;
use self::meta::HeaderValidation;
use self::meta::PosixHeader;
use self::meta::TypeFlag;

pub mod meta;
pub mod read;
pub mod write;

pub const BLOCK_SIZE: usize = 512;


#[derive(Debug, PartialEq)]
pub enum ErrorTar {
    InvalidBlockSize,
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
