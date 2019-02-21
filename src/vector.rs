/// Implementation of the MerkleTree API based on storing the whole works in
/// a vector (actually, I used a deque, it's not quite as inefficient)
/// as a complete binary tree. This is dreadfully inefficient, but
/// it was a quick way to get an API implementation up and running.
extern crate byteorder;
use super::{HashableElement, MerkleHash, MerkleTree, WitnessNode};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::VecDeque;
use std::io;

/// A node in the Vector based merkle tree. Leafs hold an element,
/// Internals hold a hash, and Empty is... yeah.
#[derive(Debug)]
enum Node<T: HashableElement> {
    Leaf(T),
    Internal(T::Hash),
    Empty,
}

impl<T: HashableElement> Node<T> {
    // Helper function to generate an internal node from the left and right
    // child hashes
    fn from_hashes(left: &T::Hash, right: &T::Hash) -> Self {
        Node::Internal(T::combine_hash(&left, &right))
    }
}

// Iterator over references to the elements in the tree. Only the leaf
// nodes are iterated.
pub struct VectorLeafIterator<'a, T: HashableElement> {
    node_iter: std::iter::Skip<std::collections::vec_deque::Iter<'a, Node<T>>>,
}

impl<'a, T: HashableElement> VectorLeafIterator<'a, T> {
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

impl<'a, T: HashableElement> Iterator for VectorLeafIterator<'a, T> {
    type Item = &'a T;
    // Unwrap the leaf node at the iterator's cursor position and return a
    // reference to it.
    //
    // # Panics:
    //  *  If the tree is in an invalid structure and a node pulled from it is
    //     not a leaf
    fn next(&mut self) -> Option<Self::Item> {
        self.node_iter.next().map(|node| {
            if let Node::Leaf(ref item) = node {
                item
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
/// This is not your most performant Merkle
/// num_nodesPrimary purpose of this implementation is to test the API and get an
/// implementation up and running for testing external trees as soon
/// as possible.
///
/// Treats the tree as fixed-height with 32 levels. Calculating the hash of an
/// element with an empty right child is done by hashing it with itself.
///
/// Design inefficiencies:
///  *  Adding a new node when the tree is full requires a bunch of insertions
///     (to the point it's cheaper just to allocate a new array and rehash)
///  *  nearly half the tree will usually contain empty nodes
///  *  related nodes for a given authentication path are scattered throughout
///     the array
pub struct VectorMerkleTree<T: HashableElement> {
    nodes: VecDeque<Node<T>>,
    tree_depth: usize,
}

impl<T: HashableElement> VectorMerkleTree<T> {
    /// Construct a new, empty merkle tree on the heap and return a Box pointer
    /// to it.
    pub fn new() -> Box<Self> {
        VectorMerkleTree::new_with_size(32)
    }

    /// Used for simpler unit tests
    fn new_with_size(tree_depth: usize) -> Box<Self> {
        Box::new(VectorMerkleTree {
            nodes: VecDeque::new(),
            tree_depth,
        })
    }

    /// Called when a new leaf was added to a complete binary tree, meaning
    /// that everything needs to be moved around and hashes need to be
    /// recalculated. The garbage in this method is the whole reason a vector
    /// based complete binary tree implementation is inefficient.
    fn rehash_all_levels(&mut self, element: T) {
        let mut new_vec = VecDeque::new();
        new_vec.push_front(Node::Leaf(element));

        let old_vec_length = self.nodes.len();
        let old_leaf_start = first_leaf(old_vec_length);

        for _ in old_leaf_start..old_vec_length {
            new_vec.push_front(self.nodes.pop_back().expect("There are more nodes"));
        }
        self.nodes = new_vec;

        // The deque currently contains all the leaf nodes, with the first leaf at index 0
        // and last leaf at the end.
        //
        // Next, all the internal nodes need to be pushed onto the front of the deque.
        // This gets confusing because we need to keep track of nodes relative to their
        // current position in the deque, as well as their final position once the deque
        // is full
        let mut index_being_added = old_vec_length - 1;
        loop {
            let left_child_in_nodes = left_child_index(index_being_added) - index_being_added - 1;

            let new_node = match (
                self.extract_hash(left_child_in_nodes),
                self.extract_hash(left_child_in_nodes + 1),
            ) {
                (None, None) => Node::Empty,
                (Some(ref hash), None) => Node::from_hashes(hash, hash),
                (Some(ref left_hash), Some(ref right_hash)) => {
                    Node::from_hashes(left_hash, right_hash)
                }
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
                (Some(ref hash), None) => T::combine_hash(hash, hash),
                (Some(ref left_hash), Some(ref right_hash)) => {
                    T::combine_hash(left_hash, right_hash)
                }
                (_, _) => {
                    panic!("Invalid tree structure");
                }
            };

            self.nodes[parent_position] = Node::Internal(parent_hash);

            current_position = parent_position;
        }
    }

    fn is_empty(&self) -> bool {
        self.nodes.len() == 0
    }

    /// Extract the hash from a leaf or internal node.
    ///
    /// Returns None if the position is invalid or empty
    fn extract_hash(&self, position: usize) -> Option<T::Hash> {
        match self.nodes.get(position) {
            None => None,
            Some(Node::Empty) => None,
            Some(Node::Leaf(ref element)) => Some(element.merkle_hash()),
            Some(Node::Internal(ref hash)) => Some(hash.clone()),
        }
    }
}

impl<T: HashableElement> MerkleTree for VectorMerkleTree<T> {
    type Element = T;
    /// Load a merkle tree from a reader and return a box pointer to it
    fn read<R: io::Read>(reader: &mut R) -> io::Result<Box<Self>> {
        let tree_depth = reader.read_u8()?;
        let num_nodes = reader.read_u32::<LittleEndian>()?;
        let mut tree = VectorMerkleTree::new_with_size(tree_depth as usize);
        for _ in 0..num_nodes {
            tree.add(T::read(reader)?);
        }
        Ok(tree)
    }

    /// Add a new element to the Merkle Tree, keeping the internal array
    /// consistent as necessary.
    ///
    /// If
    ///  *  the vector is currently a complete binary tree
    ///      *  then allocate a new vector and compute all new hashes
    ///  *  otherwise
    ///      *  append an element and update all its parent hashes
    fn add(&mut self, element: T) {
        if self.is_empty() {
            self.nodes.push_back(Node::Leaf(element));
        } else if is_complete(self.nodes.len()) {
            if num_levels(self.nodes.len()) == self.tree_depth {
                panic!("Tree is full!");
            }
            self.rehash_all_levels(element);
        } else {
            self.nodes.push_back(Node::Leaf(element));
            self.rehash_leaf_path();
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

    /// The current root hash of the tree.
    fn root_hash(&self) -> Option<T::Hash> {
        self.extract_hash(0).map(|h| {
            let extra_levels = self.tree_depth - num_levels(self.nodes.len());
            let mut cur = h;
            for _ in 0..extra_levels {
                cur = T::combine_hash(&cur, &cur)
            }
            cur
        })
    }

    /// What was the root of the tree when it had past_size leaf nodes
    fn past_root(&self, past_size: usize) -> Option<T::Hash> {
        if self.nodes.len() == 0 || past_size > self.len() {
            return None;
        }
        let mut cur = first_leaf(self.nodes.len()) + past_size - 1;
        let mut current_hash = self
            .extract_hash(cur)
            .expect("current node must be in tree");
        let mut num_levels = 1;
        while !is_leftmost_path(cur) {
            if is_left_child(cur) {
                // We're walking the right-most path, so a left child can't
                // possibly have a sibling
                current_hash = T::combine_hash(&current_hash, &current_hash);
            } else {
                let sibling_hash = self
                    .extract_hash(cur - 1)
                    .expect("Sibling node must be in tree");
                current_hash = T::combine_hash(&sibling_hash, &current_hash);
            }
            cur = parent_index(cur);
            num_levels += 1;
        }

        while num_levels < self.tree_depth {
            current_hash = T::combine_hash(&current_hash, &current_hash);
            num_levels += 1;
        }
        return Some(current_hash);
    }

    /// Construct the proof that the leaf node at `position` exists.
    ///
    /// In this implementation, we guarantee that the witness_path is
    /// tree_depth levels deep by repeatedly hashing the
    /// last root_hash with itself.
    fn witness_path(&self, position: usize) -> Option<Vec<WitnessNode<T::Hash>>> {
        if self.len() == 0 || position >= self.len() {
            return None;
        }
        let mut witnesses = vec![];
        let mut current_position = first_leaf(self.nodes.len()) + position;

        while current_position != 0 {
            if let Some(my_hash) = self.extract_hash(current_position) {
                if is_left_child(current_position) {
                    let sibling_hash = self
                        .extract_hash(current_position + 1)
                        .unwrap_or_else(|| my_hash.clone());
                    witnesses.push(WitnessNode::Left(sibling_hash));
                } else {
                    let sibling_hash = self
                        .extract_hash(current_position - 1)
                        .expect("left child must exist if right child does");
                    witnesses.push(WitnessNode::Right(sibling_hash));
                }
            } else {
                panic!("Invalid tree structure");
            }
            current_position = parent_index(current_position);
        }

        // Assuming the root hash isn't at the top of a tree that has tree_depth
        // levels, it needs to be added to the tree and hashed with itself until
        // the appropriate hash is found
        let mut sibling_hash = self.extract_hash(0).expect("Tree couldn't be empty");
        while witnesses.len() < self.tree_depth - 1 {
            witnesses.push(WitnessNode::Left(sibling_hash.clone()));
            sibling_hash = T::combine_hash(&sibling_hash, &sibling_hash);
        }

        Some(witnesses)
    }
    /// Write the vector to an array
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(self.tree_depth as u8)?;
        writer.write_u32::<LittleEndian>(self.len() as u32)?;
        for element in self {
            element.write(writer)?;
        }
        Ok(())
    }
}

impl<'a, T: HashableElement> IntoIterator for &'a VectorMerkleTree<T> {
    type Item = &'a T;
    type IntoIter = VectorLeafIterator<'a, T>;

    /// Allow a for..in over the tree. This iterates references to the tree nodes.
    /// It can only be called on a &VectorMerkleTree.
    fn into_iter(self) -> Self::IntoIter {
        VectorLeafIterator::new(&self.nodes)
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

/// The number of levels in the tree, including the last unfinished level
/// floor(log2(num_nodes)) + 1
fn num_levels(num_nodes: usize) -> usize {
    if num_nodes == 0 {
        return 0;
    }
    (num_nodes as f32).log2() as usize + 1
}

/// What is the index of the first leaf in the tree?
/// (basically (2**num_levels) / 2
fn first_leaf(num_nodes: usize) -> usize {
    if num_nodes == 0 {
        panic!("Tree is empty");
    }
    (1 << (num_levels(num_nodes) - 1)) - 1
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
mod tests {
    use super::{
        first_leaf, is_complete, is_left_child, num_levels, parent_index, Node, VectorMerkleTree,
    };
    use crate::{HashableElement, MerkleTree, WitnessNode};
    use byteorder::{ReadBytesExt, WriteBytesExt};
    use std::fmt;
    use std::io;
    use std::io::Read;

    /// Fake hashable element that just concatenates strings so it is easy to
    /// test that the correct values are output. It's weird cause the hashes are
    /// also strings. Probably best to ignore this impl and just read the tests!
    impl HashableElement for String {
        type Hash = String;
        fn merkle_hash(&self) -> Self {
            (*self).clone()
        }

        fn combine_hash(left: &String, right: &String) -> Self {
            (*left).clone() + right
        }

        fn read<R: io::Read>(reader: &mut R) -> io::Result<String> {
            let str_size = reader.read_u8()?;
            // There has GOT to be a better way to do this...
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

        fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
            let bytes = self.as_bytes();
            writer.write_u8(bytes.len() as u8)?;
            writer.write_all(bytes)?;
            Ok(())
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

    #[test]
    fn add() {
        let mut tree = VectorMerkleTree::new();
        tree.add("a".to_string());
        assert_eq!(tree.nodes.len(), 1);
        assert_matches!(tree.nodes[0], Node::Leaf(ref e) if *e == "a".to_string());
        tree.add("b".to_string());
        assert_eq!(tree.nodes.len(), 3);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[1], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[2], Node::Leaf(ref e) if *e == "b".to_string());
        tree.add("c".to_string());
        assert_eq!(tree.nodes.len(), 6);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "abcc".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "cc".to_string());
        assert_matches!(tree.nodes[3], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[4], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[5], Node::Leaf(ref e) if *e == "c".to_string());
        tree.add("d".to_string());
        assert_eq!(tree.nodes.len(), 7);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "abcd".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "cd".to_string());
        assert_matches!(tree.nodes[3], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[4], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[5], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[6], Node::Leaf(ref e) if *e == "d".to_string());
        tree.add("e".to_string());
        assert_eq!(tree.nodes.len(), 12);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "abcdeeee".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "abcd".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "eeee".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "cd".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "ee".to_string());
        assert_matches!(tree.nodes[6], Node::Empty);
        assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
        tree.add("f".to_string());
        assert_eq!(tree.nodes.len(), 13);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "abcdefef".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "abcd".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "efef".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "cd".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "ef".to_string());
        assert_matches!(tree.nodes[6], Node::Empty);
        assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
        assert_matches!(tree.nodes[12], Node::Leaf(ref e) if *e == "f".to_string());
        tree.add("g".to_string());
        assert_eq!(tree.nodes.len(), 14);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "abcdefgg".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "abcd".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "efgg".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "cd".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "ef".to_string());
        assert_matches!(tree.nodes[6], Node::Internal(ref e) if *e == "gg".to_string());
        assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
        assert_matches!(tree.nodes[12], Node::Leaf(ref e) if *e == "f".to_string());
        assert_matches!(tree.nodes[13], Node::Leaf(ref e) if *e == "g".to_string());
        tree.add("h".to_string());
        assert_eq!(tree.nodes.len(), 15);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "abcdefgh".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "abcd".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "efgh".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "cd".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "ef".to_string());
        assert_matches!(tree.nodes[6], Node::Internal(ref e) if *e == "gh".to_string());
        assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
        assert_matches!(tree.nodes[12], Node::Leaf(ref e) if *e == "f".to_string());
        assert_matches!(tree.nodes[13], Node::Leaf(ref e) if *e == "g".to_string());
        assert_matches!(tree.nodes[14], Node::Leaf(ref e) if *e == "h".to_string());
        tree.add("i".to_string());
        assert_eq!(tree.nodes.len(), 24);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "abcdefghiiiiiiii".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "abcdefgh".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "iiiiiiii".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "abcd".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "efgh".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "iiii".to_string());
        assert_matches!(tree.nodes[6], Node::Empty);
        assert_matches!(tree.nodes[7], Node::Internal(ref e) if *e == "ab".to_string());
        assert_matches!(tree.nodes[8], Node::Internal(ref e) if *e == "cd".to_string());
        assert_matches!(tree.nodes[9], Node::Internal(ref e) if *e == "ef".to_string());
        assert_matches!(tree.nodes[10], Node::Internal(ref e) if *e == "gh".to_string());
        assert_matches!(tree.nodes[11], Node::Internal(ref e) if *e == "ii".to_string());
        assert_matches!(tree.nodes[12], Node::Empty);
        assert_matches!(tree.nodes[13], Node::Empty);
        assert_matches!(tree.nodes[14], Node::Empty);
        assert_matches!(tree.nodes[15], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[16], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[17], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[18], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[19], Node::Leaf(ref e) if *e == "e".to_string());
        assert_matches!(tree.nodes[20], Node::Leaf(ref e) if *e == "f".to_string());
        assert_matches!(tree.nodes[21], Node::Leaf(ref e) if *e == "g".to_string());
        assert_matches!(tree.nodes[22], Node::Leaf(ref e) if *e == "h".to_string());
        assert_matches!(tree.nodes[23], Node::Leaf(ref e) if *e == "i".to_string());
    }

    #[test]
    fn len() {
        let mut tree = VectorMerkleTree::new();
        for i in 0..100 {
            assert_eq!(tree.len(), i);
            tree.add("a".to_string());
        }
    }

    #[test]
    fn root_hash_functions() {
        let mut tree = VectorMerkleTree::new_with_size(5);
        assert_eq!(tree.root_hash(), None);
        assert_eq!(tree.past_root(1), None);
        tree.add("a".to_string());
        assert_eq!(tree.root_hash(), Some("aaaaaaaaaaaaaaaa".to_string()));
        assert_eq!(tree.past_root(1), Some("aaaaaaaaaaaaaaaa".to_string()));
        assert_eq!(tree.past_root(2), None);
        tree.add("b".to_string());
        assert_eq!(tree.root_hash(), Some("abababababababab".to_string()));
        assert_eq!(tree.past_root(1), Some("aaaaaaaaaaaaaaaa".to_string()));
        assert_eq!(tree.past_root(2), Some("abababababababab".to_string()));
        assert_eq!(tree.past_root(3), None);
        tree.add("c".to_string());
        assert_eq!(tree.root_hash(), Some("abccabccabccabcc".to_string()));
        assert_eq!(tree.past_root(1), Some("aaaaaaaaaaaaaaaa".to_string()));
        assert_eq!(tree.past_root(2), Some("abababababababab".to_string()));
        assert_eq!(tree.past_root(3), Some("abccabccabccabcc".to_string()));
        assert_eq!(tree.past_root(4), None);
        tree.add("d".to_string());
        assert_eq!(tree.root_hash(), Some("abcdabcdabcdabcd".to_string()));
        assert_eq!(tree.past_root(1), Some("aaaaaaaaaaaaaaaa".to_string()));
        assert_eq!(tree.past_root(2), Some("abababababababab".to_string()));
        assert_eq!(tree.past_root(3), Some("abccabccabccabcc".to_string()));
        assert_eq!(tree.past_root(4), Some("abcdabcdabcdabcd".to_string()));
        assert_eq!(tree.past_root(5), None);
        for i in 0..12 {
            tree.add(i.to_string());
        }
        assert_eq!(tree.root_hash(), Some("abcd01234567891011".to_string()));
        assert_eq!(tree.past_root(1), Some("aaaaaaaaaaaaaaaa".to_string()));
        assert_eq!(tree.past_root(2), Some("abababababababab".to_string()));
        assert_eq!(tree.past_root(3), Some("abccabccabccabcc".to_string()));
        assert_eq!(tree.past_root(4), Some("abcdabcdabcdabcd".to_string()));
        assert_eq!(tree.past_root(5), Some("abcd0000abcd0000".to_string()));
        assert_eq!(tree.past_root(6), Some("abcd0101abcd0101".to_string()));
        assert_eq!(tree.past_root(7), Some("abcd0122abcd0122".to_string()));
        assert_eq!(tree.past_root(8), Some("abcd0123abcd0123".to_string()));
        assert_eq!(tree.past_root(9), Some("abcd012344444444".to_string()));
        assert_eq!(tree.past_root(10), Some("abcd012345454545".to_string()));
        assert_eq!(tree.past_root(11), Some("abcd012345664566".to_string()));
        assert_eq!(tree.past_root(12), Some("abcd012345674567".to_string()));
        assert_eq!(tree.past_root(13), Some("abcd012345678888".to_string()));
        assert_eq!(tree.past_root(14), Some("abcd012345678989".to_string()));
        assert_eq!(tree.past_root(15), Some("abcd01234567891010".to_string()));
        assert_eq!(tree.past_root(16), Some("abcd01234567891011".to_string()));
        assert_eq!(tree.past_root(17), None);
    }

    #[test]
    fn witness_path() {
        // Tree with 4 levels (8 leaves) for easier reasoning
        let mut tree = VectorMerkleTree::new_with_size(4);
        assert!(tree.witness_path(0).is_none());
        tree.add("a".to_string());
        assert!(tree.witness_path(1).is_none());
        assert_eq!(
            tree.witness_path(0).expect("path exists"),
            vec![
                WitnessNode::Left("a".to_string()),
                WitnessNode::Left("aa".to_string()),
                WitnessNode::Left("aaaa".to_string()),
            ]
        );

        tree.add("b".to_string());
        assert!(tree.witness_path(2).is_none());
        assert_eq!(
            tree.witness_path(0).expect("path exists"),
            vec![
                WitnessNode::Left("b".to_string()),
                WitnessNode::Left("ab".to_string()),
                WitnessNode::Left("abab".to_string()),
            ]
        );
        assert_eq!(
            tree.witness_path(1).expect("path exists"),
            vec![
                WitnessNode::Right("a".to_string()),
                WitnessNode::Left("ab".to_string()),
                WitnessNode::Left("abab".to_string()),
            ]
        );

        tree.add("c".to_string());
        assert!(tree.witness_path(3).is_none());
        assert_eq!(
            tree.witness_path(0).expect("path exists"),
            vec![
                WitnessNode::Left("b".to_string()),
                WitnessNode::Left("cc".to_string()),
                WitnessNode::Left("abcc".to_string()),
            ]
        );
        assert_eq!(
            tree.witness_path(1).expect("path exists"),
            vec![
                WitnessNode::Right("a".to_string()),
                WitnessNode::Left("cc".to_string()),
                WitnessNode::Left("abcc".to_string()),
            ]
        );
        assert_eq!(
            tree.witness_path(2).expect("path exists"),
            vec![
                WitnessNode::Left("c".to_string()),
                WitnessNode::Right("ab".to_string()),
                WitnessNode::Left("abcc".to_string()),
            ]
        );
        tree.add("d".to_string());
        assert!(tree.witness_path(4).is_none());
        assert_eq!(
            tree.witness_path(3).expect("path exists"),
            vec![
                WitnessNode::Right("c".to_string()),
                WitnessNode::Right("ab".to_string()),
                WitnessNode::Left("abcd".to_string()),
            ]
        );
        for i in 0..4 {
            tree.add(i.to_string());
        }
        assert!(tree.witness_path(8).is_none());
        assert_eq!(
            tree.witness_path(3).expect("path exists"),
            vec![
                WitnessNode::Right("c".to_string()),
                WitnessNode::Right("ab".to_string()),
                WitnessNode::Left("0123".to_string()),
            ]
        );
        assert_eq!(
            tree.witness_path(4).expect("path exists"),
            vec![
                WitnessNode::Left("1".to_string()),
                WitnessNode::Left("23".to_string()),
                WitnessNode::Right("abcd".to_string()),
            ]
        );
        assert_eq!(
            tree.witness_path(5).expect("path exists"),
            vec![
                WitnessNode::Right("0".to_string()),
                WitnessNode::Left("23".to_string()),
                WitnessNode::Right("abcd".to_string()),
            ]
        );
        assert_eq!(
            tree.witness_path(6).expect("path exists"),
            vec![
                WitnessNode::Left("3".to_string()),
                WitnessNode::Right("01".to_string()),
                WitnessNode::Right("abcd".to_string()),
            ]
        );
        assert_eq!(
            tree.witness_path(7).expect("path exists"),
            vec![
                WitnessNode::Right("2".to_string()),
                WitnessNode::Right("01".to_string()),
                WitnessNode::Right("abcd".to_string()),
            ]
        );
    }

    #[test]
    fn iteration() {
        let mut tree = VectorMerkleTree::new();
        let mut iter = tree.into_iter();
        assert_eq!(iter.next(), None);

        tree.add("a".to_string());
        let mut iter = tree.into_iter();
        assert_eq!(iter.next(), Some(&"a".to_string()));
        assert_eq!(iter.next(), None);

        tree.add("b".to_string());
        let mut iter = tree.into_iter();
        assert_eq!(iter.next(), Some(&"a".to_string()));
        assert_eq!(iter.next(), Some(&"b".to_string()));
        assert_eq!(iter.next(), None);

        tree.add("c".to_string());
        let mut iter = tree.into_iter();
        assert_eq!(iter.next(), Some(&"a".to_string()));
        assert_eq!(iter.next(), Some(&"b".to_string()));
        assert_eq!(iter.next(), Some(&"c".to_string()));
        assert_eq!(iter.next(), None);

        tree.add("d".to_string());
        let mut iter = tree.into_iter();
        assert_eq!(iter.next(), Some(&"a".to_string()));
        assert_eq!(iter.next(), Some(&"b".to_string()));
        assert_eq!(iter.next(), Some(&"c".to_string()));
        assert_eq!(iter.next(), Some(&"d".to_string()));
        assert_eq!(iter.next(), None);

        tree.add("e".to_string());
        let mut iter = tree.into_iter();
        assert_eq!(iter.next(), Some(&"a".to_string()));
        assert_eq!(iter.next(), Some(&"b".to_string()));
        assert_eq!(iter.next(), Some(&"c".to_string()));
        assert_eq!(iter.next(), Some(&"d".to_string()));
        assert_eq!(iter.next(), Some(&"e".to_string()));
        assert_eq!(iter.next(), None);

        tree.add("f".to_string());
        let mut iter = tree.into_iter();
        assert_eq!(iter.next(), Some(&"a".to_string()));
        assert_eq!(iter.next(), Some(&"b".to_string()));
        assert_eq!(iter.next(), Some(&"c".to_string()));
        assert_eq!(iter.next(), Some(&"d".to_string()));
        assert_eq!(iter.next(), Some(&"e".to_string()));
        assert_eq!(iter.next(), Some(&"f".to_string()));
        assert_eq!(iter.next(), None);

        for i in 0..100 {
            tree.add(i.to_string());
        }
        let mut iter = tree.into_iter();
        for char in ["a", "b", "c", "d", "e", "f"].iter() {
            assert_eq!(iter.next(), Some(&char.to_string()));
        }

        for i in 0..100 {
            assert_eq!(iter.next(), Some(&i.to_string()));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn serialization() {
        let mut tree = VectorMerkleTree::new_with_size(5);
        for i in 0..12 {
            tree.add(i.to_string());
        }
        let mut bytes = vec![];
        tree.write(&mut bytes)
            .expect("should be able to write bytes.");
        println!("{:?}", bytes);

        let read_back_tree: Box<VectorMerkleTree<String>> =
            VectorMerkleTree::read(&mut bytes[..].as_ref()).expect("should be able to read bytes.");

        let mut bytes_again = vec![];
        println!("{:?}", bytes_again);
        read_back_tree
            .write(&mut bytes_again)
            .expect("should still be able to write bytes.");
        assert_eq!(bytes, bytes_again);
    }

    #[test]
    fn private_tree_mathy_methods() {
        let mut num_nodes = 0;
        assert!(is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 0);
        // no first_leaf check, it should panic
        // no parent_index check, it should panic

        num_nodes = 1;
        assert!(is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 1);
        assert_eq!(first_leaf(num_nodes), 0);
        assert_eq!(parent_index(num_nodes), 0);
        assert!(is_left_child(num_nodes));

        num_nodes = 2;
        assert!(!is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 2);
        assert_eq!(first_leaf(num_nodes), 1);
        assert_eq!(parent_index(num_nodes), 0);
        assert!(!is_left_child(num_nodes));

        num_nodes = 3;
        assert!(is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 2);
        assert_eq!(first_leaf(num_nodes), 1);
        assert_eq!(parent_index(num_nodes), 1);

        num_nodes = 4;
        assert!(!is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 3);
        assert_eq!(first_leaf(num_nodes), 3);
        assert_eq!(parent_index(num_nodes), 1);

        num_nodes = 5;
        assert!(!is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 3);
        assert_eq!(first_leaf(num_nodes), 3);
        assert_eq!(parent_index(num_nodes), 2);

        num_nodes = 6;
        assert!(!is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 3);
        assert_eq!(first_leaf(num_nodes), 3);
        assert_eq!(parent_index(num_nodes), 2);

        num_nodes = 7;
        assert!(is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 3);
        assert_eq!(first_leaf(num_nodes), 3);
        assert_eq!(parent_index(num_nodes), 3);

        for _ in 0..7 {
            num_nodes += 1;
            assert!(!is_complete(num_nodes));
            assert_eq!(num_levels(num_nodes), 4);
            assert_eq!(first_leaf(num_nodes), 7);
        }

        num_nodes = 15;
        assert!(is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 4);
        assert_eq!(first_leaf(num_nodes), 7);
        assert_eq!(parent_index(num_nodes), 7);

        for _ in 0..15 {
            num_nodes += 1;
            assert!(!is_complete(num_nodes));
            assert_eq!(num_levels(num_nodes), 5);
            assert_eq!(first_leaf(num_nodes), 15);
        }

        num_nodes = 31;
        assert!(is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 5);
        assert_eq!(first_leaf(num_nodes), 15);
        assert_eq!(parent_index(num_nodes), 15);

        num_nodes = 32;
        assert!(!is_complete(num_nodes));
        assert_eq!(num_levels(num_nodes), 6);
        assert_eq!(first_leaf(num_nodes), 31);
        assert_eq!(parent_index(num_nodes), 15);
    }
}
