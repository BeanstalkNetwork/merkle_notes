use crate::{HashableElement, MerkleHasher, WitnessNode};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io;
use std::io::Read;
use std::sync::Arc;

/// Fake hashable element that just concatenates strings so it is easy to
/// test that the correct values are output. It's weird cause the hashes are
/// also strings. Probably best to ignore this impl and just read the tests!
impl HashableElement for String {
    type Hash = String;
    fn merkle_hash(&self) -> Self {
        (*self).clone()
    }

    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let bytes = self.as_bytes();
        writer.write_u8(bytes.len() as u8)?;
        writer.write_all(bytes)?;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct StringHasher {}

impl StringHasher {
    pub fn new() -> Arc<StringHasher> {
        Arc::new(StringHasher {})
    }
}

impl MerkleHasher for StringHasher {
    type Element = String;
    fn combine_hash(&self, depth: usize, left: &String, right: &String) -> String {
        "<".to_string() + &(*left).clone() + "|" + right + "-" + &depth.to_string() + ">"
    }

    fn read_element<R: io::Read>(&self, reader: &mut R) -> io::Result<String> {
        let str_size = reader.read_u8()?;
        // There has GOT to be a better way to do this
        // (read str_size bytes into a string)
        let bytes = reader
            .take(str_size as u64)
            .bytes()
            .map(|b| b.unwrap())
            .collect::<Vec<u8>>();
        match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "shouldn't go wrong",
            )),
        }
    }

    fn read_hash<R: io::Read>(&self, reader: &mut R) -> io::Result<String> {
        let hash_length = reader.read_u32::<LittleEndian>().unwrap();
        let mut bytes = vec![0u8; hash_length as usize];
        reader.read_exact(&mut bytes)?;
        Ok(String::from_utf8(bytes).unwrap())
    }

    fn write_hash<W: io::Write>(&self, hash: &String, writer: &mut W) -> io::Result<()> {
        let bytes = hash.as_bytes();
        writer.write_u32::<LittleEndian>(bytes.len() as u32)?;
        writer.write_all(bytes)
    }
}

impl fmt::Debug for WitnessNode<String> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WitnessNode::Left(hash) => write!(f, "Left {}", hash),
            WitnessNode::Right(hash) => write!(f, "Right {}", hash),
        }
    }
}

impl PartialEq for WitnessNode<String> {
    fn eq(&self, other: &WitnessNode<String>) -> bool {
        match (self, other) {
            (WitnessNode::Left(a), WitnessNode::Left(b)) => a == b,
            (WitnessNode::Right(a), WitnessNode::Right(b)) => a == b,
            (_, _) => false,
        }
    }
}

/// Fake hashable element that just counts the number of levels.
/// I made this because man, 32 levels of StringHasher is a lot of bytes.
/// Like, crashed my computer bytes.
impl HashableElement for u64 {
    type Hash = u64;
    fn merkle_hash(&self) -> Self {
        *self
    }

    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(*self)?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct CountHasher {}

impl CountHasher {
    pub fn new() -> Arc<CountHasher> {
        Arc::new(CountHasher {})
    }
}

impl MerkleHasher for CountHasher {
    type Element = u64;
    fn combine_hash(&self, _depth: usize, left: &u64, _right: &u64) -> u64 {
        left + 1
    }

    fn read_element<R: io::Read>(&self, reader: &mut R) -> io::Result<u64> {
        reader.read_u64::<LittleEndian>()
    }

    fn read_hash<R: io::Read>(&self, _reader: &mut R) -> io::Result<u64> {
        panic!("Not needed for the unit test suite");
    }

    fn write_hash<W: io::Write>(&self, _hash: &u64, _writer: &mut W) -> io::Result<()> {
        panic!("Not needed for the unit test suite");
    }
}
