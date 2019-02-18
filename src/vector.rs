use super::{HashableElement, MerkleHash, MerkleTree, WitnessNode};
use std::collections::VecDeque;
use std::io;

#[derive(Debug)]
enum Node<H: MerkleHash, T: HashableElement<H>> {
    Leaf(T),
    Internal(H),
    Empty,
}

impl<H: MerkleHash, T: HashableElement<H>> Node<H, T> {
    fn from_hashes(left: &H, right: &H) -> Self {
        Node::Internal(T::combine_hash(&left, &right))
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
/// Treats the tree as fixed-sized with 32 levels. Calculating the hash of an
/// element with an empty right child is done by hashing it with itself.
///
/// Design inefficiencies:
///  *  Adding a new node when the tree is full requires a bunch of insertions
///     (to the point it's cheaper just to allocate a new array and rehash)
///  *  nearly half the tree will usually contain empty nodes
///  *  related nodes for a given authentication path are scattered throughout
///     the array
pub struct VectorMerkleTree<H: MerkleHash, T: HashableElement<H>> {
    nodes: VecDeque<Node<H, T>>,
}

// TODO: This needs to fulfill the entire interface
//impl<H, T: HashableElement<H>> MerkleTree<H, T> for VectorMerkleTree<H, T> {
impl<H: MerkleHash, T: HashableElement<H>> VectorMerkleTree<H, T> {
    /// Construct a new, empty merkle tree on the heap and return a Box pointer
    /// to it.
    pub fn new() -> Box<Self> {
        Box::new(VectorMerkleTree {
            nodes: VecDeque::new(),
        })
    }

    /// Load a merkle tree from a reader and return a box pointer to it
    pub fn read<R: io::Read>(&self, reader: &mut R) -> io::Result<Box<Self>> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Not implemented yet",
        ))
    }

    /// Add a new element to the Merkle Tree, keeping the internal array
    /// consistent as necessary.
    ///
    /// If
    ///  *  the vector is currently a complete binary tree
    ///      *  then allocate a new vector and compute all new hashes
    ///  *  otherwise
    ///      *  append an element and update all its parent hashes
    pub fn add(&mut self, element: T) {
        if self.is_empty() {
            self.nodes.push_back(Node::Leaf(element));
        } else if is_complete(self.nodes.len()) {
            self.rehash_all_levels(element);
        } else {
            self.nodes.push_back(Node::Leaf(element));
            self.rehash_leaf_path();
        }
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
        let old_num_leaves = old_vec_length - old_leaf_start;
        let new_vec_length = old_vec_length + old_num_leaves + 1;

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
            // Both children of being added are outside bounds of list
            if left_child_in_nodes >= self.nodes.len() {
                self.nodes.push_front(Node::Empty);
            }
            // Right child of new node is outside bounds of list, but left is in it
            else if left_child_in_nodes == self.nodes.len() - 1 {
                if let Node::Leaf(ref leaf_element) = self.nodes[left_child_in_nodes] {
                    let leaf_hash = leaf_element.merkle_hash();
                    self.nodes
                        .push_front(Node::from_hashes(&leaf_hash, &leaf_hash));
                } else {
                    panic!("last child should be a leaf or this loop wouldn't have been entered");
                }
            } else {
                // both children are in the list
                let new_node = match (
                    &self.nodes[left_child_in_nodes],
                    &self.nodes[left_child_in_nodes + 1],
                ) {
                    (Node::Empty, Node::Empty) => Node::Empty,
                    (Node::Leaf(ref left), Node::Empty) => {
                        Node::from_hashes(&left.merkle_hash(), &left.merkle_hash())
                    }
                    (Node::Leaf(ref left), Node::Leaf(ref right)) => {
                        Node::from_hashes(&left.merkle_hash(), &right.merkle_hash())
                    }
                    (Node::Internal(ref hash), Node::Empty) => Node::from_hashes(hash, hash),
                    (Node::Internal(ref left), Node::Internal(ref right)) => {
                        Node::from_hashes(left, right)
                    }
                    (_, _) => panic!("Invalid tree structure"),
                };
                self.nodes.push_front(new_node);
            }

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
                left = self.nodes.get(current_position);
                right = self.nodes.get(current_position + 1);
            } else {
                left = self.nodes.get(current_position - 1);
                right = self.nodes.get(current_position);
            }

            let parent_hash = match (left, right) {
                (Some(Node::Leaf(ref element)), None) => {
                    T::combine_hash(&element.merkle_hash(), &element.merkle_hash())
                }
                (Some(Node::Leaf(ref element)), Some(Node::Empty)) => {
                    T::combine_hash(&element.merkle_hash(), &element.merkle_hash())
                }
                (Some(Node::Leaf(ref left_elem)), Some(Node::Leaf(ref right_elem))) => {
                    T::combine_hash(&left_elem.merkle_hash(), &right_elem.merkle_hash())
                }
                (Some(Node::Internal(ref hash)), None) => T::combine_hash(hash, hash),
                (Some(Node::Internal(ref hash)), Some(Node::Empty)) => T::combine_hash(hash, hash),
                (Some(Node::Internal(ref left_hash)), Some(Node::Internal(ref right_hash))) => {
                    T::combine_hash(left_hash, right_hash)
                }
                (a, b) => {
                    panic!("Invalid tree structure");
                }
            };

            self.nodes[parent_position] = Node::Internal(parent_hash);

            current_position = parent_position;
        }

        //let parent_position = parent_po
    }

    fn is_empty(&self) -> bool {
        self.nodes.len() == 0
    }
}

/// Is it a complete binary tree that would need a new level if we added
/// a node? (It's complete if the number of nodes is a power of two)
fn is_complete(num_nodes: usize) -> bool {
    let level_counter = num_nodes + 1;
    level_counter & (level_counter - 1) == 0
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
    use crate::{HashableElement, MerkleHash};

    impl MerkleHash for String {}

    impl HashableElement<String> for String {
        fn merkle_hash(&self) -> Self {
            (*self).clone()
        }

        fn combine_hash(left: &String, right: &String) -> Self {
            (*left).clone() + right
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

/*
Complete binary tree math:
 *  Number of levels: floor(log2(num_nodes)) + 1
 *  Number of internal nodes: (2**num_levels) / 2 - 1
 *  Number of leaves: num_nodes - num_internal
 *  Am I currently a complete binary tree?
     * num_nodes + 1 is a power of two
 *  If my index is:
     * 0 -> I am root
     * odd -> I am a left child
     * even -> I am a right child
 *  My sibling's index is:
     *  my_index + 1 if I am odd
     *  my_index - 1 if I am even
 *  My children's indices are:
     * left child: (my_index + 1) * 2 - 1
     * right child: (my_index + 1) * 2
 *  My parent's index is:
     * ceil(my_index / 2) -1
*/
