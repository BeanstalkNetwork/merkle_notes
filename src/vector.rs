/// Implementation of the MerkleTree API based on storing the whole works in
/// a vector (actually, I used a deque, it's not quite as inefficient)
/// as a complete binary tree. This is dreadfully inefficient, but
/// it was a quick way to get an API implementation up and running.
extern crate byteorder;
use super::{HashableElement, MerkleHasher, MerkleTree, Witness, WitnessNode};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;

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
/// This is not your most performant Merkle
/// num_nodesPrimary purpose of this implementation is to test the API and get an
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
    hasher: Rc<T>,
}

impl<T: MerkleHasher> VectorMerkleTree<T> {
    /// Used for simpler unit tests
    fn new_with_size(hasher: Rc<T>, tree_depth: usize) -> Box<Self> {
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

    /// Construct a new, empty merkle tree on the heap and return a Box pointer
    /// to it.
    fn new(hasher: Rc<T>) -> Box<Self> {
        VectorMerkleTree::new_with_size(hasher, 33)
    }

    /// Load a merkle tree from a reader and return a box pointer to it
    fn read<R: io::Read>(hasher: Rc<T>, reader: &mut R) -> io::Result<Box<Self>> {
        let tree_depth = reader.read_u8()?;
        let num_nodes = reader.read_u32::<LittleEndian>()?;
        let mut tree = VectorMerkleTree::new_with_size(hasher, tree_depth as usize);
        for _ in 0..num_nodes {
            tree.add(tree.hasher.read_element(reader)?);
        }
        Ok(tree)
    }

    /// Expose the hasher
    fn hasher(&self) -> Rc<T> {
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
    ) -> Box<Iterator<Item = <Self::Hasher as MerkleHasher>::Element> + 'a> {
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
mod tests {
    use super::{
        depth_at_index, first_leaf, first_leaf_by_num_leaves, is_complete, is_left_child,
        parent_index, Node, VectorMerkleTree,
    };
    use crate::{HashableElement, MerkleHasher, MerkleTree, WitnessNode};
    use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
    use std::fmt;
    use std::io;
    use std::io::Read;
    use std::rc::Rc;

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

    #[derive(Debug)]
    struct StringHasher {}

    impl StringHasher {
        fn new() -> Rc<StringHasher> {
            Rc::new(StringHasher {})
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

        fn read_hash<R: io::Read>(&self, _reader: &mut R) -> io::Result<String> {
            panic!("Not needed for the unit test suite");
        }

        fn write_hash<W: io::Write>(&self, _hash: &String, _writer: &mut W) -> io::Result<()> {
            panic!("Not needed for the unit test suite");
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
    struct CountHasher {}

    impl CountHasher {
        fn new() -> Rc<CountHasher> {
            Rc::new(CountHasher {})
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

    #[test]
    fn add() {
        let mut tree = VectorMerkleTree::new(StringHasher::new());
        tree.add("a".to_string());
        assert_eq!(tree.nodes.len(), 1);
        assert_matches!(tree.nodes[0], Node::Leaf(ref e) if *e == "a".to_string());
        tree.add("b".to_string());
        assert_eq!(tree.nodes.len(), 3);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[1], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[2], Node::Leaf(ref e) if *e == "b".to_string());
        tree.add("c".to_string());
        assert_eq!(tree.nodes.len(), 6);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<a|b-0>|<c|c-0>-1>".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<c|c-0>".to_string());
        assert_matches!(tree.nodes[3], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[4], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[5], Node::Leaf(ref e) if *e == "c".to_string());
        tree.add("d".to_string());
        assert_eq!(tree.nodes.len(), 7);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
        assert_matches!(tree.nodes[3], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[4], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[5], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[6], Node::Leaf(ref e) if *e == "d".to_string());
        tree.add("e".to_string());
        assert_eq!(tree.nodes.len(), 12);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|e-0>|<e|e-0>-1>-2>".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|e-0>|<e|e-0>-1>".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|e-0>".to_string());
        assert_matches!(tree.nodes[6], Node::Empty);
        assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
        tree.add("f".to_string());
        assert_eq!(tree.nodes.len(), 13);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<e|f-0>-1>-2>".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|f-0>|<e|f-0>-1>".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
        assert_matches!(tree.nodes[6], Node::Empty);
        assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
        assert_matches!(tree.nodes[12], Node::Leaf(ref e) if *e == "f".to_string());
        tree.add("g".to_string());
        assert_eq!(tree.nodes.len(), 14);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|g-0>-1>-2>".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|f-0>|<g|g-0>-1>".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
        assert_matches!(tree.nodes[6], Node::Internal(ref e) if *e == "<g|g-0>".to_string());
        assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
        assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
        assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
        assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
        assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
        assert_matches!(tree.nodes[12], Node::Leaf(ref e) if *e == "f".to_string());
        assert_matches!(tree.nodes[13], Node::Leaf(ref e) if *e == "g".to_string());
        tree.add("h".to_string());
        assert_eq!(tree.nodes.len(), 15);
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|f-0>|<g|h-0>-1>".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
        assert_matches!(tree.nodes[6], Node::Internal(ref e) if *e == "<g|h-0>".to_string());
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
        assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>|<<<i|i-0>|<i|i-0>-1>|<<i|i-0>|<i|i-0>-1>-2>-3>".to_string());
        assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>".to_string());
        assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<<i|i-0>|<i|i-0>-1>|<<i|i-0>|<i|i-0>-1>-2>".to_string());
        assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
        assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<<e|f-0>|<g|h-0>-1>".to_string());
        assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<<i|i-0>|<i|i-0>-1>".to_string());
        assert_matches!(tree.nodes[6], Node::Empty);
        assert_matches!(tree.nodes[7], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
        assert_matches!(tree.nodes[8], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
        assert_matches!(tree.nodes[9], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
        assert_matches!(tree.nodes[10], Node::Internal(ref e) if *e == "<g|h-0>".to_string());
        assert_matches!(tree.nodes[11], Node::Internal(ref e) if *e == "<i|i-0>".to_string());
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
        let mut tree = VectorMerkleTree::new(StringHasher::new());
        for i in 0..100 {
            assert_eq!(tree.len(), i);
            tree.add("a".to_string());
        }
    }

    #[test]
    fn root_hash_functions() {
        let mut tree = VectorMerkleTree::new_with_size(StringHasher::new(), 5);
        assert_eq!(tree.root_hash(), None);
        assert_eq!(tree.past_root(1), None);
        tree.add("a".to_string());
        assert_eq!(
            tree.root_hash(),
            Some(
                "<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>"
                    .to_string()
            )
        );
        assert_eq!(tree.past_root(1), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(2), None);
        tree.add("b".to_string());
        assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(1), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(2), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(3), None);
        tree.add("c".to_string());
        assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>|<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(1), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(2), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(3), Some("<<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>|<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(4), None);
        tree.add("d".to_string());
        assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(1), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(2), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(3), Some("<<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>|<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(4), Some("<<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(5), None);
        for i in 0..12 {
            tree.add(i.to_string());
        }
        assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|7-0>-1>|<<8|9-0>|<10|11-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(1), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(2), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(3), Some("<<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>|<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(4), Some("<<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(5), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|0-0>|<0|0-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<0|0-0>|<0|0-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(6), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<0|1-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<0|1-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(7), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|2-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|2-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(8), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(9), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|4-0>|<4|4-0>-1>|<<4|4-0>|<4|4-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(10), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<4|5-0>-1>|<<4|5-0>|<4|5-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(11), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|6-0>-1>|<<4|5-0>|<6|6-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(12), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|7-0>-1>|<<4|5-0>|<6|7-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(13), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|7-0>-1>|<<8|8-0>|<8|8-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(14), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|7-0>-1>|<<8|9-0>|<8|9-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(15), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|7-0>-1>|<<8|9-0>|<10|10-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(16), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|7-0>-1>|<<8|9-0>|<10|11-0>-1>-2>-3>".to_string()));
        assert_eq!(tree.past_root(17), None);
    }

    #[test]
    fn witness_path() {
        let hasher = StringHasher::new();
        // Tree with 4 levels (8 leaves) for easier reasoning
        let mut tree = VectorMerkleTree::new_with_size(hasher, 4);
        assert!(tree.witness(0).is_none());

        tree.add("a".to_string());
        assert!(tree.witness(1).is_none());
        let mut expected_root = "<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>";
        let mut witness = tree.witness(0).expect("path exists");
        assert!(witness.verify(&tree.hasher, &"a".to_string()));
        assert!(!witness.verify(&tree.hasher, &"b".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(witness.tree_size, 1);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Left("a".to_string()),
                WitnessNode::Left("<a|a-0>".to_string()),
                WitnessNode::Left("<<a|a-0>|<a|a-0>-1>".to_string()),
            ]
        );

        tree.add("b".to_string());
        expected_root = "<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>";
        assert!(tree.witness(2).is_none());
        witness = tree.witness(0).expect("path exists");
        assert_eq!(witness.tree_size, 2);
        assert!(witness.verify(&tree.hasher, &"a".to_string()));
        assert!(!witness.verify(&tree.hasher, &"b".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Left("b".to_string()),
                WitnessNode::Left("<a|b-0>".to_string()),
                WitnessNode::Left("<<a|b-0>|<a|b-0>-1>".to_string()),
            ]
        );
        witness = tree.witness(1).expect("path exists");
        assert_eq!(witness.tree_size, 2);
        assert!(witness.verify(&tree.hasher, &"b".to_string()));
        assert!(!witness.verify(&tree.hasher, &"a".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Right("a".to_string()),
                WitnessNode::Left("<a|b-0>".to_string()),
                WitnessNode::Left("<<a|b-0>|<a|b-0>-1>".to_string()),
            ]
        );

        tree.add("c".to_string());
        expected_root = "<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>";
        assert!(tree.witness(3).is_none());
        witness = tree.witness(0).expect("path exists");
        assert_eq!(witness.tree_size, 3);
        assert!(witness.verify(&tree.hasher, &"a".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Left("b".to_string()),
                WitnessNode::Left("<c|c-0>".to_string()),
                WitnessNode::Left("<<a|b-0>|<c|c-0>-1>".to_string()),
            ]
        );
        witness = tree.witness(1).expect("path exists");
        assert_eq!(witness.tree_size, 3);
        assert!(witness.verify(&tree.hasher, &"b".to_string()));
        assert_eq!(witness.root_hash, expected_root);

        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Right("a".to_string()),
                WitnessNode::Left("<c|c-0>".to_string()),
                WitnessNode::Left("<<a|b-0>|<c|c-0>-1>".to_string()),
            ]
        );
        witness = tree.witness(2).expect("path exists");
        assert_eq!(witness.tree_size, 3);
        assert!(witness.verify(&tree.hasher, &"c".to_string()));
        assert_eq!(witness.root_hash, expected_root);

        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Left("c".to_string()),
                WitnessNode::Right("<a|b-0>".to_string()),
                WitnessNode::Left("<<a|b-0>|<c|c-0>-1>".to_string()),
            ]
        );
        tree.add("d".to_string());
        expected_root = "<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>";
        witness = tree.witness(3).expect("path exists");
        assert_eq!(witness.tree_size, 4);
        assert_eq!(witness.root_hash, expected_root);
        assert!(witness.verify(&tree.hasher, &"d".to_string()));
        assert!(tree.witness(4).is_none());
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Right("c".to_string()),
                WitnessNode::Right("<a|b-0>".to_string()),
                WitnessNode::Left("<<a|b-0>|<c|d-0>-1>".to_string()),
            ]
        );
        for i in 0..4 {
            tree.add(i.to_string());
        }
        expected_root = "<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>";
        assert!(tree.witness(8).is_none());
        witness = tree.witness(3).expect("path exists");
        assert_eq!(witness.tree_size, 8);
        assert!(witness.verify(&tree.hasher, &"d".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Right("c".to_string()),
                WitnessNode::Right("<a|b-0>".to_string()),
                WitnessNode::Left("<<0|1-0>|<2|3-0>-1>".to_string()),
            ]
        );
        witness = tree.witness(4).expect("path exists");
        assert_eq!(witness.tree_size, 8);
        assert!(witness.verify(&tree.hasher, &"0".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Left("1".to_string()),
                WitnessNode::Left("<2|3-0>".to_string()),
                WitnessNode::Right("<<a|b-0>|<c|d-0>-1>".to_string()),
            ]
        );
        witness = tree.witness(5).expect("path exists");
        assert_eq!(witness.tree_size, 8);
        assert!(witness.verify(&tree.hasher, &"1".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Right("0".to_string()),
                WitnessNode::Left("<2|3-0>".to_string()),
                WitnessNode::Right("<<a|b-0>|<c|d-0>-1>".to_string()),
            ]
        );
        witness = tree.witness(6).expect("path exists");
        assert_eq!(witness.tree_size, 8);
        assert!(witness.verify(&tree.hasher, &"2".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Left("3".to_string()),
                WitnessNode::Right("<0|1-0>".to_string()),
                WitnessNode::Right("<<a|b-0>|<c|d-0>-1>".to_string()),
            ]
        );
        witness = tree.witness(7).expect("path exists");
        assert_eq!(witness.tree_size, 8);
        assert!(witness.verify(&tree.hasher, &"3".to_string()));
        assert_eq!(witness.root_hash, expected_root);
        assert_eq!(
            witness.auth_path,
            vec![
                WitnessNode::Right("2".to_string()),
                WitnessNode::Right("<0|1-0>".to_string()),
                WitnessNode::Right("<<a|b-0>|<c|d-0>-1>".to_string()),
            ]
        );
    }

    #[test]
    fn test_truncate() {
        let mut tree = VectorMerkleTree::new_with_size(StringHasher::new(), 5);
        tree.truncate(0);
        tree.truncate(1);

        tree.add("a".to_string());
        tree.truncate(1);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.nodes.len(), 1);
        assert_eq!(tree.iter_notes().next(), Some("a".to_string()));
        assert_eq!(tree.root_hash(), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
        tree.truncate(0);
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.nodes.len(), 0);
        assert!(tree.root_hash().is_none());

        tree.add("a".to_string());
        tree.add("b".to_string());
        tree.truncate(2);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
        tree.truncate(1);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.iter_notes().next(), Some("a".to_string()));
        assert_eq!(tree.root_hash(), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));

        tree.add("b".to_string());
        tree.add("c".to_string());
        tree.add("d".to_string());
        tree.add("e".to_string());
        tree.add("f".to_string());
        tree.add("g".to_string());
        tree.add("h".to_string());
        tree.add("i".to_string());
        tree.truncate(5); // abcde
        assert_eq!(tree.len(), 5);
        let mut iter = tree.iter_notes();
        assert_eq!(iter.next(), Some("a".to_string()));
        assert_eq!(iter.next(), Some("b".to_string()));
        assert_eq!(iter.next(), Some("c".to_string()));
        assert_eq!(iter.next(), Some("d".to_string()));
        assert_eq!(iter.next(), Some("e".to_string()));
        assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|d-0>-1>|<<e|e-0>|<e|e-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<e|e-0>|<e|e-0>-1>-2>-3>".to_string()));
    }

    #[test]
    fn iteration() {
        let mut tree = VectorMerkleTree::new(StringHasher::new());
        {
            let mut iter = tree.iter_notes();
            assert_eq!(iter.next(), None);
        }

        tree.add("a".to_string());
        {
            let mut iter = tree.iter_notes();
            assert_eq!(iter.next(), Some("a".to_string()));
            assert_eq!(iter.next(), None);
        }

        {
            tree.add("b".to_string());
            let mut iter = tree.iter_notes();
            assert_eq!(iter.next(), Some("a".to_string()));
            assert_eq!(iter.next(), Some("b".to_string()));
            assert_eq!(iter.next(), None);
        }
        {
            tree.add("c".to_string());
            let mut iter = tree.iter_notes();
            assert_eq!(iter.next(), Some("a".to_string()));
            assert_eq!(iter.next(), Some("b".to_string()));
            assert_eq!(iter.next(), Some("c".to_string()));
            assert_eq!(iter.next(), None);
        }
        {
            tree.add("d".to_string());
            let mut iter = tree.iter_notes();
            assert_eq!(iter.next(), Some("a".to_string()));
            assert_eq!(iter.next(), Some("b".to_string()));
            assert_eq!(iter.next(), Some("c".to_string()));
            assert_eq!(iter.next(), Some("d".to_string()));
            assert_eq!(iter.next(), None);
        }
        {
            tree.add("e".to_string());
            let mut iter = tree.iter_notes();
            assert_eq!(iter.next(), Some("a".to_string()));
            assert_eq!(iter.next(), Some("b".to_string()));
            assert_eq!(iter.next(), Some("c".to_string()));
            assert_eq!(iter.next(), Some("d".to_string()));
            assert_eq!(iter.next(), Some("e".to_string()));
            assert_eq!(iter.next(), None);
        }
        {
            tree.add("f".to_string());
            let mut iter = tree.iter_notes();
            assert_eq!(iter.next(), Some("a".to_string()));
            assert_eq!(iter.next(), Some("b".to_string()));
            assert_eq!(iter.next(), Some("c".to_string()));
            assert_eq!(iter.next(), Some("d".to_string()));
            assert_eq!(iter.next(), Some("e".to_string()));
            assert_eq!(iter.next(), Some("f".to_string()));
            assert_eq!(iter.next(), None);
        }

        for i in 0..100 {
            tree.add(i.to_string());
        }
        let mut iter = tree.iter_notes();
        for char in ["a", "b", "c", "d", "e", "f"].iter() {
            assert_eq!(iter.next(), Some(char.to_string()));
        }

        for i in 0..100 {
            assert_eq!(iter.next(), Some(i.to_string()));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn serialization() {
        let mut tree = VectorMerkleTree::new_with_size(StringHasher::new(), 5);
        for i in 0..12 {
            tree.add(i.to_string());
        }
        let mut bytes = vec![];
        tree.write(&mut bytes)
            .expect("should be able to write bytes.");

        let read_back_tree: Box<VectorMerkleTree<StringHasher>> =
            VectorMerkleTree::read(StringHasher::new(), &mut bytes[..].as_ref())
                .expect("should be able to read bytes.");

        let mut bytes_again = vec![];
        read_back_tree
            .write(&mut bytes_again)
            .expect("should still be able to write bytes.");
        assert_eq!(bytes, bytes_again);
    }

    #[test]
    fn test_depth_at_index() {
        assert_eq!(depth_at_index(0), 1);
        assert_eq!(depth_at_index(1), 2);
        assert_eq!(depth_at_index(2), 2);
        assert_eq!(depth_at_index(3), 3);
        assert_eq!(depth_at_index(4), 3);
        assert_eq!(depth_at_index(5), 3);
        assert_eq!(depth_at_index(6), 3);
        assert_eq!(depth_at_index(7), 4);
        assert_eq!(depth_at_index(8), 4);
        assert_eq!(depth_at_index(9), 4);
        assert_eq!(depth_at_index(10), 4);
        assert_eq!(depth_at_index(11), 4);
        assert_eq!(depth_at_index(12), 4);
        assert_eq!(depth_at_index(13), 4);
        assert_eq!(depth_at_index(14), 4);
        assert_eq!(depth_at_index(15), 5);
        assert_eq!(depth_at_index(16), 5);
        assert_eq!(depth_at_index(30), 5);
        assert_eq!(depth_at_index(31), 6);
        assert_eq!(depth_at_index(62), 6);
        assert_eq!(depth_at_index(63), 7);
        assert_eq!(depth_at_index(127), 8);
    }

    #[test]
    fn test_first_leaf() {
        assert_eq!(first_leaf(1), 0);
        assert_eq!(first_leaf(2), 1);
        assert_eq!(first_leaf(3), 1);
        assert_eq!(first_leaf(4), 3);
        assert_eq!(first_leaf(5), 3);
        assert_eq!(first_leaf(6), 3);
        assert_eq!(first_leaf(7), 3);
        assert_eq!(first_leaf(8), 7);
        assert_eq!(first_leaf(9), 7);
        assert_eq!(first_leaf(10), 7);
        assert_eq!(first_leaf(11), 7);
        assert_eq!(first_leaf(12), 7);
        assert_eq!(first_leaf(13), 7);
        assert_eq!(first_leaf(14), 7);
        assert_eq!(first_leaf(15), 7);
        assert_eq!(first_leaf(16), 15);
        assert_eq!(first_leaf(31), 15);
        assert_eq!(first_leaf(63), 31);
        assert_eq!(first_leaf(64), 63);
    }

    #[test]
    fn test_first_leaf_by_num_leaves() {
        for i in 1..18 {
            println!(
                "{} {} {}",
                i,
                depth_at_index(i),
                first_leaf_by_num_leaves(i)
            );
        }
        assert_eq!(first_leaf_by_num_leaves(1), 0);
        assert_eq!(first_leaf_by_num_leaves(2), 1);
        assert_eq!(first_leaf_by_num_leaves(3), 3);
        assert_eq!(first_leaf_by_num_leaves(4), 3);
        assert_eq!(first_leaf_by_num_leaves(5), 7);
        assert_eq!(first_leaf_by_num_leaves(6), 7);
        assert_eq!(first_leaf_by_num_leaves(7), 7);
        assert_eq!(first_leaf_by_num_leaves(8), 7);
        assert_eq!(first_leaf_by_num_leaves(9), 15);
        assert_eq!(first_leaf_by_num_leaves(10), 15);
        assert_eq!(first_leaf_by_num_leaves(11), 15);
        assert_eq!(first_leaf_by_num_leaves(12), 15);
        assert_eq!(first_leaf_by_num_leaves(13), 15);
        assert_eq!(first_leaf_by_num_leaves(14), 15);
        assert_eq!(first_leaf_by_num_leaves(15), 15);
        assert_eq!(first_leaf_by_num_leaves(16), 15);
        assert_eq!(first_leaf_by_num_leaves(17), 31);
        assert_eq!(first_leaf_by_num_leaves(32), 31);
        assert_eq!(first_leaf_by_num_leaves(33), 63);
        assert_eq!(first_leaf_by_num_leaves(64), 63);
        assert_eq!(first_leaf_by_num_leaves(65), 127);
        assert_eq!(first_leaf_by_num_leaves(128), 127);
    }

    #[test]
    fn private_tree_mathy_methods() {
        let mut num_nodes = 0;
        assert!(is_complete(num_nodes));
        assert_eq!(depth_at_index(num_nodes), 1);
        // no parent_index check, it should panic

        num_nodes = 1;
        assert!(is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 0);
        assert!(is_left_child(num_nodes));

        num_nodes = 2;
        assert!(!is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 0);
        assert!(!is_left_child(num_nodes));

        num_nodes = 3;
        assert!(is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 1);

        num_nodes = 4;
        assert!(!is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 1);

        num_nodes = 5;
        assert!(!is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 2);

        num_nodes = 6;
        assert!(!is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 2);

        num_nodes = 7;
        assert!(is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 3);

        for _ in 0..7 {
            num_nodes += 1;
            assert!(!is_complete(num_nodes));
        }

        num_nodes = 15;
        assert!(is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 7);

        for _ in 0..15 {
            num_nodes += 1;
            assert!(!is_complete(num_nodes));
        }

        num_nodes = 31;
        assert!(is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 15);

        num_nodes = 32;
        assert!(!is_complete(num_nodes));
        assert_eq!(parent_index(num_nodes), 15);
    }

    #[test]
    fn default_authpath_len() {
        let mut tree = VectorMerkleTree::new(CountHasher::new());
        tree.add(1);
        let witness = tree.witness(0).expect("node exists");
        assert_eq!(witness.root_hash, 33);
        assert_eq!(witness.auth_path.len(), 32);
    }
}
