/// Implementation of the MerkleTree API based on storing the whole works in
/// a vector (actually, I used a deque, it's not quite as inefficient)
/// as a complete binary tree. This is dreadfully inefficient, but
/// it was a quick way to get an API implementation up and running.
use super::{HashableElement, MerkleHasher, MerkleTree, Witness, WitnessNode};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::VecDeque;
use std::io;
use std::sync::Arc;

/// A node in the Vector based merkle tree. Leafs hold an element,
/// Internals hold a hash, and Empty is... yeah.
#[derive(Debug)]
enum Node<T: MerkleHasher> {
    Leaf(T::Element),
    Internal(<T::Element as HashableElement>::Hash),
    Empty,
}

// Iterator over references to the elements in the tree. Only the leaf
// nodes are iterated.
pub struct VectorLeafIterator<'a, T: MerkleHasher> {
    node_iter: std::iter::Skip<std::collections::vec_deque::Iter<'a, Node<T>>>,
}

impl<'a, T: MerkleHasher> VectorLeafIterator<'a, T> {
    // Construct a new iterator using a reference to the nodes in a VectorMerkleTree
    fn new(nodes: &'a VecDeque<Node<T>>) -> VectorLeafIterator<'a, T> {
        let first_leaf_index = if nodes.len() > 0 {
            first_leaf(nodes.len())
        } else {
            0
        };
        let node_iter = nodes.iter().skip(first_leaf_index);
        VectorLeafIterator { node_iter }
    }
}

impl<'a, T: MerkleHasher> Iterator for VectorLeafIterator<'a, T> {
    type Item = T::Element;
    // Unwrap the leaf node at the iterator's cursor position and return a
    // reference to it.
    //
    // # Panics:
    //  *  If the tree is in an invalid structure and a node pulled from it is
    //     not a leaf
    fn next(&mut self) -> Option<Self::Item> {
        self.node_iter.next().map(|node| {
            if let Node::Leaf(ref item) = node {
                item.clone()
            } else {
                panic!("Expect all leaf nodes in order");
            }
        })
    }
}

/// Basic implementation of the MerkleTree trait. The tree is stored in a
/// vector as a complete binary tree, and index calculations are used to
/// look up nodes relative to each other.
///
/// This is not your most performant Merkle tree.
/// The purpose of this implementation is to test the API and get an
/// implementation up and running for testing external trees as soon
/// as possible.
///
/// Treats the tree as fixed-height with 33 levels, including the root node. This
/// causes the auth_path to have 32 levels, which is what sapling expects (
/// apparently their `tree_depth` does not include the root node)
/// Calculating the hash of an element with an empty right child is
/// done by hashing it with itself.
///
/// Design inefficiencies:
///  *  Adding a new node when the tree is full requires a bunch of insertions
///     (to the point it's cheaper just to allocate a new array and rehash)
///  *  nearly half the tree will usually contain empty nodes
///  *  related nodes for a given authentication path are scattered throughout
///     the array
pub struct VectorMerkleTree<T: MerkleHasher> {
    nodes: VecDeque<Node<T>>,
    tree_depth: usize,
    hasher: Arc<T>,
}

impl<T: MerkleHasher> VectorMerkleTree<T> {
    /// Construct a new, empty merkle tree on the heap and return a Box pointer
    /// to it.
    pub fn new(hasher: Arc<T>) -> Box<Self> {
        VectorMerkleTree::new_with_size(hasher, 33)
    }

    /// Used for simpler unit tests
    fn new_with_size(hasher: Arc<T>, tree_depth: usize) -> Box<Self> {
        Box::new(VectorMerkleTree {
            nodes: VecDeque::new(),
            tree_depth,
            hasher,
        })
    }

    /// Called when a new leaf was added to a complete binary tree, meaning
    /// that everything needs to be moved around and hashes need to be
    /// recalculated. The garbage in this method is the whole reason a vector
    /// based complete binary tree implementation is inefficient.
    fn add_leaf_rehash(&mut self, element: T::Element) {
        let old_leaf_start = first_leaf(self.nodes.len());
        self.nodes.push_back(Node::Leaf(element));

        for _ in 0..old_leaf_start {
            self.nodes.pop_front();
        }

        self.rehash_all_levels();
    }

    /// Assuming self.nodes contains only leaf nodes, rebuild the hashes of all the
    /// internal nodes.
    ///
    /// The deque currently contains all the leaf nodes, with the first leaf at index 0
    /// and last leaf at the end.
    ///
    /// Next, all the internal nodes need to be pushed onto the front of the deque.
    /// This gets confusing because we need to keep track of nodes relative to their
    /// current position in the deque, as well as their final position once the deque
    /// is full
    fn rehash_all_levels(&mut self) {
        let num_internal = first_leaf_by_num_leaves(self.nodes.len());
        if num_internal == 0 {
            return;
        }
        let internal_depth = depth_at_index(num_internal - 1);

        let mut index_being_added = num_internal - 1;
        loop {
            let child_node_depth = internal_depth - depth_at_index(index_being_added);
            let left_child_in_nodes = left_child_index(index_being_added) - index_being_added - 1;

            let new_node = match (
                self.extract_hash(left_child_in_nodes),
                self.extract_hash(left_child_in_nodes + 1),
            ) {
                (None, None) => Node::Empty,
                (Some(ref hash), None) => {
                    Node::Internal(self.hasher.combine_hash(child_node_depth, hash, hash))
                }
                (Some(ref left_hash), Some(ref right_hash)) => Node::Internal(
                    self.hasher
                        .combine_hash(child_node_depth, left_hash, right_hash),
                ),
                (_, _) => panic!("Invalid tree structure"),
            };
            self.nodes.push_front(new_node);

            if index_being_added == 0 {
                break;
            }
            index_being_added -= 1;
        }
    }

    fn rehash_leaf_path(&mut self) {
        let mut current_position = self.nodes.len() - 1;
        let mut depth = 0;

        while current_position != 0 {
            let parent_position = parent_index(current_position);
            let left;
            let right;
            if is_left_child(current_position) {
                left = self.extract_hash(current_position);
                right = self.extract_hash(current_position + 1);
            } else {
                left = self.extract_hash(current_position - 1);
                right = self.extract_hash(current_position);
            }

            let parent_hash = match (left, right) {
                (Some(ref hash), None) => self.hasher.combine_hash(depth, hash, hash),
                (Some(ref left_hash), Some(ref right_hash)) => {
                    self.hasher.combine_hash(depth, left_hash, right_hash)
                }
                (_, _) => {
                    panic!("Invalid tree structure");
                }
            };

            self.nodes[parent_position] = Node::Internal(parent_hash);

            depth += 1;
            current_position = parent_position;
        }
    }

    fn is_empty(&self) -> bool {
        self.nodes.len() == 0
    }

    /// Extract the hash from a leaf or internal node.
    ///
    /// Returns None if the position is invalid or empty
    fn extract_hash(&self, position: usize) -> Option<<T::Element as HashableElement>::Hash> {
        match self.nodes.get(position) {
            None => None,
            Some(Node::Empty) => None,
            Some(Node::Leaf(ref element)) => Some(element.merkle_hash()),
            Some(Node::Internal(ref hash)) => Some(hash.clone()),
        }
    }
}

impl<T: MerkleHasher> MerkleTree for VectorMerkleTree<T> {
    type Hasher = T;
    /// Load a merkle tree from a reader and return a box pointer to it
    fn read<R: io::Read>(hasher: Arc<T>, reader: &mut R) -> io::Result<Box<Self>> {
        let tree_depth = reader.read_u8()?;
        let num_nodes = reader.read_u32::<LittleEndian>()?;
        let mut tree = VectorMerkleTree::new_with_size(hasher, tree_depth as usize);
        for _ in 0..num_nodes {
            tree.add(tree.hasher.read_element(reader)?);
        }
        Ok(tree)
    }

    /// Expose the hasher
    fn hasher(&self) -> Arc<T> {
        self.hasher.clone()
    }

    /// Add a new element to the Merkle Tree, keeping the internal array
    /// consistent as necessary.
    ///
    /// If
    ///  *  the vector is currently a complete binary tree
    ///      *  then allocate a new vector and compute all new hashes
    ///  *  otherwise
    ///      *  append an element and update all its parent hashes
    fn add(&mut self, element: T::Element) {
        if self.is_empty() {
            self.nodes.push_back(Node::Leaf(element));
        } else if is_complete(self.nodes.len()) {
            if depth_at_index(self.nodes.len()) == self.tree_depth + 1 {
                panic!("Tree is full!");
            }
            self.add_leaf_rehash(element);
        } else {
            self.nodes.push_back(Node::Leaf(element));
            self.rehash_leaf_path();
        }
    }

    /// Get the leaf note at a specific position
    fn get(&self, position: usize) -> Option<<Self::Hasher as MerkleHasher>::Element> {
        if self.nodes.len() == 0 {
            return None;
        }
        let position = first_leaf(self.nodes.len()) + position;
        match self.nodes.get(position) {
            Some(Node::Leaf(element)) => Some(element.clone()),
            _ => None,
        }
    }

    /// Get the number of leaf nodes in the tree
    fn len(&self) -> usize {
        if self.nodes.len() == 0 {
            0
        } else {
            self.nodes.len() - first_leaf(self.nodes.len())
        }
    }

    /// Truncate the tree to when it was a specific past size.
    fn truncate(&mut self, past_size: usize) {
        if past_size >= self.len() {
            return;
        }
        if past_size == 0 {
            self.nodes.clear();
            return;
        }

        let old_leaf_start = first_leaf(self.nodes.len());

        for _ in 0..self.len() - past_size {
            self.nodes.pop_back();
        }

        for _ in 0..old_leaf_start {
            self.nodes.pop_front();
        }

        self.rehash_all_levels();
    }

    /// Iterate over clones of all leaf notes in the tree, without consuming
    /// the tree.
    fn iter_notes<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = <Self::Hasher as MerkleHasher>::Element> + 'a> {
        Box::new(VectorLeafIterator::new(&self.nodes))
    }

    /// The current root hash of the tree.
    fn root_hash(&self) -> Option<<T::Element as HashableElement>::Hash> {
        self.extract_hash(0).map(|h| {
            let mut cur = h;
            for i in depth_at_index(self.nodes.len() - 1)..self.tree_depth {
                cur = self.hasher.combine_hash(i - 1, &cur, &cur)
            }
            cur
        })
    }

    /// What was the root of the tree when it had past_size leaf nodes
    fn past_root(&self, past_size: usize) -> Option<<T::Element as HashableElement>::Hash> {
        if self.nodes.len() == 0 || past_size > self.len() {
            return None;
        }
        let mut cur = first_leaf(self.nodes.len()) + past_size - 1;
        let mut current_hash = self
            .extract_hash(cur)
            .expect("current node must be in tree");
        let mut depth = 0;
        while !is_leftmost_path(cur) {
            if is_left_child(cur) {
                // We're walking the right-most path, so a left child can't
                // possibly have a sibling
                current_hash = self
                    .hasher
                    .combine_hash(depth, &current_hash, &current_hash);
            } else {
                let sibling_hash = self
                    .extract_hash(cur - 1)
                    .expect("Sibling node must be in tree");
                current_hash = self
                    .hasher
                    .combine_hash(depth, &sibling_hash, &current_hash);
            }
            cur = parent_index(cur);
            depth += 1;
        }

        while depth < self.tree_depth - 1 {
            current_hash = self
                .hasher
                .combine_hash(depth, &current_hash, &current_hash);
            depth += 1;
        }
        return Some(current_hash);
    }

    /// Did the tree contain the given element when it was the given size?
    ///
    /// Uses a slow linear scan. Not... efficient.
    fn contained(&self, value: &T::Element, past_size: usize) -> bool {
        for (idx, candidate) in self.iter_notes().enumerate() {
            if idx == past_size {
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
    /// In this implementation, we guarantee that the witness_path is
    /// tree_depth levels deep by repeatedly hashing the
    /// last root_hash with itself.
    fn witness(&self, position: usize) -> Option<Witness<T>> {
        if self.len() == 0 || position >= self.len() {
            return None;
        }
        let mut auth_path = vec![];
        let mut current_position = first_leaf(self.nodes.len()) + position;
        let mut depth = 0;

        while current_position != 0 {
            if let Some(my_hash) = self.extract_hash(current_position) {
                if is_left_child(current_position) {
                    let sibling_hash = self
                        .extract_hash(current_position + 1)
                        .unwrap_or_else(|| my_hash.clone());
                    auth_path.push(WitnessNode::Left(sibling_hash));
                } else {
                    let sibling_hash = self
                        .extract_hash(current_position - 1)
                        .expect("left child must exist if right child does");
                    auth_path.push(WitnessNode::Right(sibling_hash));
                }
            } else {
                panic!("Invalid tree structure");
            }
            current_position = parent_index(current_position);
            depth += 1;
        }

        // Assuming the root hash isn't at the top of a tree that has tree_depth
        // levels, it needs to be added to the tree and hashed with itself until
        // the appropriate hash is found
        let mut sibling_hash = self.extract_hash(0).expect("Tree couldn't be empty");
        while auth_path.len() < self.tree_depth - 1 {
            auth_path.push(WitnessNode::Left(sibling_hash.clone()));
            sibling_hash = self
                .hasher
                .combine_hash(depth, &sibling_hash, &sibling_hash);
            depth += 1;
        }

        Some(Witness {
            auth_path,
            root_hash: self.root_hash().expect("Non-empty tree must have root"),
            tree_size: self.len(),
        })
    }
    /// Write the vector to an array
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(self.tree_depth as u8)?;
        writer.write_u32::<LittleEndian>(self.len() as u32)?;
        for element in self.iter_notes() {
            element.write(writer)?;
        }
        Ok(())
    }
}

/// Is it a complete binary tree that would need a new level if we added
/// a node? (It's complete if the number of nodes is a power of two)
fn is_complete(num_nodes: usize) -> bool {
    let level_counter = num_nodes + 1;
    level_counter & (level_counter - 1) == 0
}

/// any node that is "furthest left" in the tree. It is a left-child itself
/// and all its parent nodes are also left children It's the same math as
/// is_complete, since its a zero-indexed list, but I've given it a new name
/// for legibility.
fn is_leftmost_path(my_index: usize) -> bool {
    is_complete(my_index)
}

/// The depth of the tree at index num_nodes
///
/// floor(log2(num_nodes)) + 1
fn depth_at_index(index: usize) -> usize {
    ((index + 1) as f32).log2() as usize + 1
}

/// What is the index of the first leaf a tree with num_nodes elements
/// (basically (2**depth_at_index) / 2
fn first_leaf(num_nodes: usize) -> usize {
    if num_nodes == 0 {
        panic!("Tree is empty");
    }
    (1 << depth_at_index(num_nodes - 1) - 1) - 1
}

/// What is the index of the first leaf of a tree with num_leaves leaves
/// (basically (2 ** (depth_at_index - 2)) - 1)
fn first_leaf_by_num_leaves(num_leaves: usize) -> usize {
    match num_leaves {
        0 => panic!("Tree is empty"),
        1 => 0,
        _ => (1 << (depth_at_index(num_leaves - 2))) - 1,
    }
}

/// Get the index of my node's left child. The right child is always
/// left_child_index + 1
fn left_child_index(my_index: usize) -> usize {
    (my_index + 1) * 2 - 1
}

/// Get the index of my node's parent
fn parent_index(my_index: usize) -> usize {
    if my_index == 0 {
        panic!("Has no parents");
    }
    (((my_index as f32) / 2.0).ceil() as usize) - 1
}

fn is_left_child(my_index: usize) -> bool {
    my_index % 2 != 0
}
#[cfg(test)]
mod tests;
