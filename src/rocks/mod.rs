use super::{HashableElement, MerkleHasher, MerkleTree, Witness, WitnessNode};
use std::{io, sync::Arc};
mod rocker;
use rocker::{Leaf, LeafIndex, Node, NodeIndex, Rocker};

/// Merkle tree implementation stored in RocksDB. Based on LinkedMerkleTree,
/// but data isn't stored wholly in memory, and is saved incrementally,
/// instead of only on shutdown.
///
/// Note: All of the rocksdb operations get unwrap()ed because the merkle tree
/// trait doesn't permit returning Results. This is obviously not safe...
///
/// There is no transaction support in the Rust RocksDB wrapper. This
/// module is not recommended; use sled instead, even though it is beta.
pub struct RocksMerkleTree<T: MerkleHasher> {
    hasher: Arc<T>,
    rocker: Rocker<T>,
    tree_depth: u32,
}

impl<T: MerkleHasher> RocksMerkleTree<T> {
    /// Construct a new, empty merkle tree in the given directory, with
    /// a default size suitable for sapling crypto transactions.
    pub fn new(hasher: Arc<T>, rocks_directory: &std::path::Path) -> Self {
        Self::new_with_size(hasher, rocks_directory, 33)
    }

    /// Construct a new, empty merkle tree in the given directory with
    /// the given size
    pub fn new_with_size(
        hasher: Arc<T>,
        rocks_directory: &std::path::Path,
        tree_depth: u32,
    ) -> Self {
        let rocker = Rocker::new(hasher.clone(), rocks_directory);
        RocksMerkleTree {
            hasher,
            rocker,
            tree_depth: tree_depth - 1,
        }
    }

    /// Recalculate all the hashes between the most recently added leaf in the group
    /// and the root hash.
    fn rehash_right_path(&mut self) {
        let mut depth = 0;
        let leaf_index = LeafIndex(self.rocker.num_leaves() - 1);
        let leaf = self.rocker.get_leaf_metadata(leaf_index).unwrap();
        let mut parent_index = self.rocker.get_leaf_parent(leaf_index);
        let mut parent_hash = if leaf_index.is_right() {
            let sibling = self.rocker.get_leaf_metadata(leaf_index.sibling()).unwrap();
            self.hasher.combine_hash(depth, &sibling.hash, &leaf.hash)
        } else {
            self.hasher.combine_hash(depth, &leaf.hash, &leaf.hash)
        };
        loop {
            let node = self.rocker.get_node(parent_index);
            depth += 1;
            match node {
                Node::Empty => break,
                Node::Left { parent, .. } => {
                    // since we are walking the rightmost path, left nodes do not have
                    // right children. Therefore its sibling hash can be set to
                    // its own hash in its parent half will be set to the combination of
                    // that hash with itself
                    self.rocker.set_node(
                        parent_index,
                        &Node::Left {
                            parent,
                            hash_of_sibling: parent_hash.clone(),
                        },
                    );
                    parent_index = parent;
                    parent_hash = self.hasher.combine_hash(depth, &parent_hash, &parent_hash);
                }
                Node::Right {
                    left,
                    hash_of_sibling,
                } => {
                    // since this is a new right node we know that we have the correct hash
                    // because we set it correctly when we inserted it. But our left node
                    // needs to have its hash_of_sibling set to our current hash.
                    parent_index = self.rocker.get_node_parent(left);
                    self.rocker.set_node(
                        left,
                        &Node::Left {
                            parent: parent_index,
                            hash_of_sibling: parent_hash.clone(),
                        },
                    );
                    parent_hash = self
                        .hasher
                        .combine_hash(depth, &hash_of_sibling, &parent_hash);
                }
            }
        }
    }
}

impl<T: MerkleHasher> MerkleTree for RocksMerkleTree<T> {
    type Hasher = T;
    /// db writes happen on demand, so there's no need to do them here.
    fn write<W: io::Write>(&self, _writer: &mut W) -> io::Result<()> {
        Ok(())
    }

    /// Doesn't make sense to read a database that isn't stored in memory.
    /// It's not possible to fake this because we don't have access to the
    /// dbfile here.
    fn read<R: io::Read>(_hasher: Arc<T>, _reader: &mut R) -> io::Result<Box<Self>> {
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
    /// It needs to. See https://github.com/rust-rocksdb/rust-rocksdb/pull/250
    fn add(&mut self, element: T::Element) {
        let index_of_new_leaf = LeafIndex(self.rocker.num_leaves());
        if index_of_new_leaf.0 as usize >= 2_usize.pow(self.tree_depth as u32) {
            panic!("Tree is full");
        }

        let leaf_hash = element.merkle_hash();
        let new_parent_index = if index_of_new_leaf == 0 {
            // special case where this is the first leaf, with no parent
            NodeIndex::empty()
        } else if index_of_new_leaf == 1 {
            // special case where this is the second leaf, and both leaves need a new parent
            let left_leaf_index = index_of_new_leaf.sibling(); // it's 0
            let mut left_leaf = self
                .rocker
                .get_leaf_metadata(left_leaf_index)
                .expect("must have node left of the new right node");
            let new_parent_index = NodeIndex(1);
            let hash_of_sibling = self.hasher.combine_hash(0, &left_leaf.hash, &leaf_hash);
            let new_parent_of_both = Node::Left {
                parent: NodeIndex::empty(),
                hash_of_sibling,
            };
            left_leaf.parent = new_parent_index;
            self.rocker.set_num_nodes(2);
            self.rocker.set_node(new_parent_index, &new_parent_of_both);
            self.rocker.set_leaf_metadata(left_leaf_index, &left_leaf);
            new_parent_index
        } else if index_of_new_leaf.is_right() {
            // simple case where we are adding a new node to parent with an empty right child
            self.rocker.get_leaf_parent(index_of_new_leaf.sibling())
        } else {
            // Walk up the path from the previous leaf until find empty or right-hand node.
            // Create a bunch of left-hand nodes for each step up that path.
            let previous_leaf_index = LeafIndex(index_of_new_leaf.0 - 1);
            let mut next_node_index = self.rocker.num_nodes();
            let new_parent_index = NodeIndex(next_node_index);
            let mut previous_parent_index = self.rocker.get_leaf_parent(previous_leaf_index);
            let mut my_hash = self.hasher.combine_hash(0, &leaf_hash, &leaf_hash);
            let mut depth = 1;
            loop {
                let previous_parent = self.rocker.get_node(previous_parent_index);
                match previous_parent {
                    Node::Left {
                        hash_of_sibling,
                        parent,
                    } => {
                        let new_node = Node::Right {
                            left: previous_parent_index,
                            hash_of_sibling: hash_of_sibling.clone(),
                        };
                        self.rocker.set_node(NodeIndex(next_node_index), &new_node);
                        next_node_index += 1;
                        self.rocker.set_num_nodes(next_node_index);
                        if parent.is_empty() {
                            let new_parent = Node::Left {
                                parent: NodeIndex::empty(),
                                hash_of_sibling: self.hasher.combine_hash(
                                    depth,
                                    &hash_of_sibling,
                                    &my_hash,
                                ),
                            };
                            self.rocker
                                .set_node(NodeIndex(next_node_index), &new_parent);
                            self.rocker.set_node(
                                previous_parent_index,
                                &Node::Left {
                                    hash_of_sibling,
                                    parent: NodeIndex(next_node_index),
                                },
                            );
                            next_node_index += 1;
                            self.rocker.set_num_nodes(next_node_index);
                        }
                        break;
                    }
                    Node::Right { left, .. } => {
                        my_hash = self.hasher.combine_hash(depth, &my_hash, &my_hash);
                        let new_node = Node::Left {
                            parent: NodeIndex(next_node_index + 1), // This is where the next node *WILL* go
                            hash_of_sibling: my_hash.clone(),
                        };
                        self.rocker.set_node(NodeIndex(next_node_index), &new_node);
                        next_node_index += 1;
                        self.rocker.set_num_nodes(next_node_index);
                        previous_parent_index = self.rocker.get_node_parent(left);
                        depth += 1;
                    }
                    Node::Empty => unimplemented!(),
                }
            }
            new_parent_index
        };
        let new_leaf = Leaf {
            parent: new_parent_index,
            hash: leaf_hash,
        };
        self.rocker.set_num_leaves(index_of_new_leaf.0 + 1);
        self.rocker.set_leaf_metadata(index_of_new_leaf, &new_leaf);
        self.rocker.set_leaf_element(index_of_new_leaf, &element);

        self.rehash_right_path();
    }

    /// Get the leaf element at the given position.
    fn get(&self, position: usize) -> Option<<Self::Hasher as MerkleHasher>::Element> {
        self.rocker.get_leaf_element(LeafIndex(position as u32))
    }

    /// Truncate the tree to the values it contained when it contained past_size
    /// elements.
    ///
    /// After calling, it will contain at most past_size elements, but truncating
    /// to a size that is higher than self.len() is a no-op.
    ///
    /// This function doesn't do any garbage collection. The old leaves and
    /// nodes stay in rocksdb, but they will be overwritten as the tree grows.
    ///
    /// TODO: This needs to run inside a rocksdb transaction.
    fn truncate(&mut self, past_size: usize) {
        if past_size >= self.len() {
            return;
        }

        self.rocker.set_num_leaves(past_size as u32);
        if past_size == 0 {
            self.rocker.set_num_nodes(1); // The empty node
            return;
        } else if past_size == 1 {
            let mut first_leaf = self.rocker.get_leaf_metadata(LeafIndex(0)).unwrap();
            first_leaf.parent = NodeIndex::empty();
            self.rocker.set_leaf_metadata(LeafIndex(0), &first_leaf);
            self.rocker.set_num_nodes(1);
            return;
        }
        let depth = depth_at_leaf_count(past_size) - 2;
        let mut parent = self.rocker.get_leaf_parent(LeafIndex(past_size as u32 - 1));
        let mut max_parent = parent;
        for _ in 0..depth {
            parent = self.rocker.get_node_parent(parent);
            if parent.0 > max_parent.0 {
                max_parent = parent;
            }
        }

        match self.rocker.get_node(parent) {
            Node::Left {
                hash_of_sibling, ..
            } => self.rocker.set_node(
                parent,
                &Node::Left {
                    hash_of_sibling,
                    parent: NodeIndex::empty(),
                },
            ),
            _ => panic!("New root expected to be a left node."),
        }

        self.rocker.set_num_nodes(max_parent.0 + 1);
        self.rehash_right_path();
    }

    /// Iterate over clones of all leaf notes in the tree, without consuming
    /// the tree.
    ///
    /// note: this is completely undefined behaviour if the tree is modified
    /// while iteration is happening.
    fn iter_notes<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = <Self::Hasher as MerkleHasher>::Element> + 'a> {
        let leaf_count = self.rocker.num_leaves();
        Box::new(
            (0..leaf_count)
                .map(move |index| self.rocker.get_leaf_element(LeafIndex(index)).unwrap()),
        )
    }

    /// Get the hash of the current root element in the tree.
    fn root_hash(
        &self,
    ) -> Option<<<Self::Hasher as MerkleHasher>::Element as HashableElement>::Hash> {
        self.past_root(self.len())
    }

    /// Calculate what the root hash was at the time the tree contained
    /// `past_size` elements. Returns none if the tree is empty or
    /// the requested size is greater than the length of the tree.
    fn past_root(
        &self,
        past_size: usize,
    ) -> Option<<<Self::Hasher as MerkleHasher>::Element as HashableElement>::Hash> {
        if self.len() == 0 || past_size > self.len() || past_size == 0 {
            return None;
        }
        let root_depth = depth_at_leaf_count(past_size);
        let leaf_index = LeafIndex(past_size as u32 - 1);
        let mut current_hash;
        let mut current_node_index = self.rocker.get_leaf_parent(leaf_index);
        if leaf_index.is_right() {
            current_hash = self.hasher.combine_hash(
                0,
                &self
                    .rocker
                    .get_leaf_metadata(leaf_index.sibling())
                    .unwrap()
                    .hash,
                &self.rocker.get_leaf_metadata(leaf_index).unwrap().hash,
            );
        } else {
            current_hash = self.hasher.combine_hash(
                0,
                &self.rocker.get_leaf_metadata(leaf_index).unwrap().hash,
                &self.rocker.get_leaf_metadata(leaf_index).unwrap().hash,
            );
        }

        for depth in 1..std::cmp::min(root_depth, self.tree_depth as usize) {
            match self.rocker.get_node(current_node_index) {
                Node::Empty => panic!("depth should not reach empty node"),
                Node::Left { parent, .. } => {
                    current_hash = self
                        .hasher
                        .combine_hash(depth, &current_hash, &current_hash);
                    current_node_index = parent;
                }
                Node::Right {
                    left,
                    hash_of_sibling,
                } => {
                    current_hash = self
                        .hasher
                        .combine_hash(depth, &hash_of_sibling, &current_hash);
                    current_node_index = self.rocker.get_node_parent(left);
                }
            }
        }
        for depth in root_depth..(self.tree_depth as usize) {
            current_hash = self
                .hasher
                .combine_hash(depth, &current_hash, &current_hash);
        }
        Some(current_hash)
    }

    /// Determine whether a tree contained a value in the past, when it had a specific size.
    ///
    /// This is an inefficient linear scan.
    fn contained(&self, value: &T::Element, past_size: usize) -> bool {
        for (index, candidate) in self.iter_notes().enumerate() {
            if index == past_size {
                break;
            }
            if candidate == *value {
                return true;
            }
        }
        false
    }

    /// Construct the proof that the leaf node at `position` exists.
    ///
    /// The length of the returned vector is the depth of the leaf node in the
    /// tree minus 1.
    ///
    /// The leftmost value in the vector, the hash at index 0, is the hash
    /// of the leaf node's sibling. The rightmost value in the vector contains
    /// the hash of the child of the root node.
    ///
    /// The root hash is not included in the authentication path.
    fn witness(&self, position: usize) -> Option<Witness<T>> {
        if self.len() == 0 || position >= self.len() {
            return None;
        }
        let leaf_index = LeafIndex(position as u32);
        let leaf_data = self.rocker.get_leaf_metadata(leaf_index).unwrap();
        let mut current_hash = leaf_data.hash;
        let mut current_position = leaf_data.parent;
        let mut authentication_path = vec![];
        if leaf_index.is_right() {
            let sibling_hash = self.rocker.get_leaf_hash(leaf_index.sibling());
            current_hash = self.hasher.combine_hash(0, &sibling_hash, &current_hash);
            authentication_path.push(WitnessNode::Right(sibling_hash));
        } else if position < self.len() - 1 {
            // I am a left leaf and I have a right sibling
            let sibling_hash = self.rocker.get_leaf_hash(leaf_index.sibling());
            current_hash = self.hasher.combine_hash(0, &current_hash, &sibling_hash);
            authentication_path.push(WitnessNode::Left(sibling_hash));
        } else {
            // I am a left leaf and the rightmost node
            authentication_path.push(WitnessNode::Left(current_hash.clone()));
            current_hash = self.hasher.combine_hash(0, &current_hash, &current_hash);
        }
        for depth in 1..self.tree_depth as usize {
            match self.rocker.get_node(current_position) {
                Node::Empty => {
                    authentication_path.push(WitnessNode::Left(current_hash.clone()));
                    current_hash = self
                        .hasher
                        .combine_hash(depth, &current_hash, &current_hash);
                }
                Node::Left {
                    parent,
                    hash_of_sibling,
                } => {
                    authentication_path.push(WitnessNode::Left(hash_of_sibling.clone()));
                    current_hash = self
                        .hasher
                        .combine_hash(depth, &current_hash, &hash_of_sibling);
                    current_position = parent;
                }
                Node::Right {
                    left,
                    hash_of_sibling,
                } => {
                    authentication_path.push(WitnessNode::Right(hash_of_sibling.clone()));
                    current_hash = self
                        .hasher
                        .combine_hash(depth, &hash_of_sibling, &current_hash);
                    current_position = self.rocker.get_node_parent(left);
                }
            }
        }
        Some(Witness {
            auth_path: authentication_path,
            root_hash: self.root_hash().expect("nonempty must have root hash"),
            tree_size: self.len(),
        })
    }
}

/// The depth of the tree when it contains a certain
/// number of leaf nodes
///
/// floor(log2(n-1))+2
fn depth_at_leaf_count(index: usize) -> usize {
    match index {
        0 => 0,
        1 => 1,
        n => ((n - 1) as f32).log2() as usize + 2,
    }
}

#[cfg(test)]
mod tests;
