use super::{HashableElement, MerkleHasher, MerkleTree};
use config;
use std::{io, path::PathBuf, sync::Arc};

mod rocker;
use rocker::{LeafIndex, NodeIndex, Rocker};

/// Merkle tree implementation stored in RocksDB. Based on LinkedMerkleTree,
/// but data isn't stored wholly in memory, and is saved incrementally,
/// instead of only on shutdown.
///
/// Note: All of the rocksdb operations get unwrap()ed because the merkle tree
/// trait doesn't permit returning Results. This is obviously not safe...
pub struct RocksMerkleTree<T: MerkleHasher> {
    hasher: Arc<T>,
    rocker: Rocker<T>,
    tree_depth: u32,
}

impl<T: MerkleHasher> RocksMerkleTree<T> {
    pub fn new(hasher: Arc<T>, rocks_directory: &std::path::Path) -> Self {
        Self::new_with_size(hasher, rocks_directory, 33)
    }
    pub fn new_with_size(
        hasher: Arc<T>,
        rocks_directory: &std::path::Path,
        tree_depth: u32,
    ) -> Self {
        let rocker = Rocker::new(hasher.clone(), rocks_directory);
        RocksMerkleTree {
            hasher,
            rocker,
            tree_depth,
        }
    }
}

impl<T: MerkleHasher> MerkleTree for RocksMerkleTree<T> {
    // db writes happen on demand, so there's no need to do them here.
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        Ok(())
    }

    // Doesn't make sense to read a database that isn't stored in memory
    fn read<R: io::Read>(hasher: Arc<T>, reader: &mut R) -> io::Result<Box<Self>> {
        unimplemented!("Construct database using new() and let rocksdb load the content");
    }

    /// Expose the hasher
    fn hasher(&self) -> Arc<T> {
        self.hasher.clone()
    }

    /// Get the number of leaf nodes in the tree
    fn len(&self) -> usize {
        self.rocker.num_leaves() as usize
    }

    /// Add a new element to the Merkle Tree, keeping all the hashes consistent.
    ///
    /// The leaf contents and internal node hashes are all stored in rocksdb,
    /// with their relative positions.
    ///
    /// TODO: This method does not operate inside a transaction because the
    /// Rust implementation of rocksdb does not support transactions yet.
    /// It needs to.
    fn add(&mut self, element: T::Element) {
        let index_of_new_leaf = LeafIndex(self.rocker.num_leaves());
        if index_of_new_leaf.0 >= 2_u32.pow(self.tree_depth) {
            panic!("Tree is full");
        }
        let leaf_hash = element.merkle_hash();

        let new_parent_index = if index_of_new_leaf == 0 {
            NodeIndex(0)
        } else if index_of_new_leaf.is_right() {
            self.rocker.get_leaf_parent(index_of_new_leaf.sibling())
        } else {
            NodeIndex(self.rocker.num_nodes())
        };

        // TODO: Remember to increment leaf_index and node_index appropriately
    }
}

/*
 +  db of leaf nodes by positional index
 +  db of internal nodes by positional index
 + store number of leaf nodes and internal nodes in meta keys
 +  each leaf has two fields:
     +  Index of its parent
     +  the bytes containing its element
     +  (whether it is left or right node can be inferred from index, as can index of its sibling)
 +  each internal node has fields:
     +  type (left, right, parent_of_root)
     +  index of the sibling node (may be empty if this is a left node)
     +  index of the parent node
 +  db keys:
 +  LeafXX where XX is integer
 +  InternalXX where XX is integer
 +  NumLeaves as integer
 +  NumInternalNodes as integer
 +  integer is 4 byte integer little endian
*/

#[cfg(test)]
mod tests;
