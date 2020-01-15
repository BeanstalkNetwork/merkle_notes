use super::{HashableElement, MerkleHasher, MerkleTree};
use bincode;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use rocksdb::{DBPinnableSlice, Options, DB};
use std::{path::Path, sync::Arc};

const LEAF_COUNT_KEY: &str = "LeafCount";
const NODE_COUNT_KEY: &str = "NodeCount";
const LEAF_DATA_PREFIX: &[u8; 8] = b"LeafData";
const LEAF_ELEMENT_PREFIX: &[u8; 11] = b"LeafElement";

/// Newtype wrapper to avoid mixing up leaf and node indexes
#[derive(Shrinkwrap, Debug, PartialEq)]
pub(crate) struct LeafIndex(pub(crate) u32);

impl LeafIndex {
    fn data_key(&self) -> Vec<u8> {
        let key = LEAF_DATA_PREFIX.to_owned().to_vec();
        key.extend(u32_as_bytes(self.0));
        key
    }
}

impl PartialEq<u32> for LeafIndex {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl LeafIndex {
    pub(crate) fn is_right(&self) -> bool {
        self.0 % 2 == 1
    }

    /// Get the index of the sibling index. Note that this
    /// makes no guarantees as to whether the sibling leaf exists.
    pub(crate) fn sibling(&self) -> LeafIndex {
        if self.is_right() {
            LeafIndex(self.0 - 1)
        } else {
            LeafIndex(self.0 + 1)
        }
    }
}

/// Newtype wrapper to avoid mixing up leaf and node indexes
#[derive(Shrinkwrap, Debug, PartialEq)]
pub(crate) struct NodeIndex(pub(crate) u32);

/// Metadata about a leaf stored in rocksdb under the LeafData
/// key prefix. The element bytes are stored under a separate key.
#[derive(Debug, PartialEq)]
pub(crate) struct Leaf<T: MerkleHasher> {
    parent: NodeIndex,
    hash: <T::Element as HashableElement>::Hash,
}

/// Rocksdb wrapper that queries and unwraps requests for specific
/// keys and types useful to the RocksMerkleTree.
///
/// This struct uses expect() everywhere. That's not particularly safe,
/// but I don't have a better idea. We can't expose the errors upward
/// because the MerkleTree trait doesn't have Results on it, and even if we could,
/// it's unclear how the client code would handle it. So we panic... *sigh*
pub(crate) struct Rocker<T: MerkleHasher> {
    hasher: Arc<T>,
    rocksdb: DB,
}

impl<T: MerkleHasher> Rocker<T> {
    pub(crate) fn new(hasher: Arc<T>, rocks_directory: &Path) -> Self {
        Rocker {
            hasher,
            rocksdb: DB::open_default(rocks_directory).expect("Unable to load database"),
        }
    }

    /// Retrieve the number of leaf nodes (notes) in the tree
    pub(crate) fn num_leaves(&self) -> u32 {
        self.get_u32(LEAF_COUNT_KEY).unwrap_or(0)
    }

    /// Set the number of leaf nodes. It may be good to have an atomic increment
    /// operation here, since it only ever goes up by one.
    pub(crate) fn set_num_leaves(&self, length: u32) {
        self.set_u32(LEAF_COUNT_KEY, length as u32);
    }

    /// Get the number of internal nodes.
    pub(crate) fn num_nodes(&self) -> u32 {
        self.get_u32(NODE_COUNT_KEY).unwrap_or(0)
    }

    pub(crate) fn set_num_nodes(&self, count: u32) {
        self.set_u32(NODE_COUNT_KEY, count as u32);
    }

    /// Get the parent of the leaf node at given index.
    /// **Assumes that the leaf index actually exists in the tree.**
    /// This is a shortcut method when you know you'll unwrap the result
    pub(crate) fn get_leaf_parent(&self, index: LeafIndex) -> NodeIndex {
        self.get_leaf(index).unwrap().parent
    }

    fn get_u32(&self, key: &str) -> Option<u32> {
        self.get(key, |bytes| bytes.read_u32::<LittleEndian>().unwrap())
    }

    fn set_u32(&self, key: &str, value: u32) {
        let mut bytes = vec![];
        bytes.write_u32::<LittleEndian>(value).unwrap();
        self.rocksdb.put(key, bytes).unwrap();
    }

    fn get_leaf(&self, index: LeafIndex) -> Option<Leaf<T>> {
        self.get(index.data_key(), |bytes| {
            let parent = NodeIndex(bytes.read_u32::<LittleEndian>().unwrap());
            let hash = self.hasher.read_hash(&mut bytes).unwrap();
            Leaf { parent, hash }
        })
    }

    fn set_leaf(&self, index: LeafIndex, value: Leaf<T>) {
        let mut bytes = vec![];
        bytes.write_u32::<LittleEndian>(value.parent.0).unwrap();
        self.hasher.write_hash(&value.hash, &mut bytes).unwrap();
    }

    fn get<K: AsRef<[u8]>, V, F: FnOnce(&[u8]) -> V>(&self, key: K, callback: F) -> Option<V> {
        self.rocksdb
            .get_pinned(key)
            .unwrap()
            .map(|pinnable_slice| callback(pinnable_slice.as_ref()))
    }
}

fn u32_as_bytes(value: u32) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];
    bytes.write_u32::<LittleEndian>(value);
    bytes
}
