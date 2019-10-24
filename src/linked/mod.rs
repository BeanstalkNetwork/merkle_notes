use crate::{HashableElement, MerkleHasher, MerkleTree};
use std::sync::Arc;

/// Newtype wrapper of u32. It just represents an index into a vector,
#[derive(Shrinkwrap, Debug, Clone, Copy, PartialEq, PartialOrd)]
struct NodeIndex(u32);

impl NodeIndex {
    fn empty() -> Self {
        NodeIndex(0)
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl From<usize> for NodeIndex {
    fn from(value: usize) -> NodeIndex {
        NodeIndex(value as u32)
    }
}

impl std::ops::Add<u32> for NodeIndex {
    type Output = NodeIndex;

    fn add(self, other: u32) -> NodeIndex {
        NodeIndex(self.0 + other)
    }
}

/// Represent an internal node in the tree of nodes. To make it easy
/// to create an authentication path, internal nodes store the hash of their
/// *sibling* rather than their own hash.
///
/// The nodes are all stored in a vector in LinkedMerkleTree. I use u32 indices
/// into that vector instead of pointers to other nodes for a few reasons:
/// 1) On a 64-bit system, u32 uses half as much memory as a pointer,
/// and with the number of nodes we expect, that's an appreciable amount of memory.
/// 2) Maintaining a tree of linked nodes using safe Rust is unpleasant.
/// 3) Something something storing the tree in contiguous memory.
#[derive(Debug, PartialEq)]
enum InternalNode<T: MerkleHasher> {
    /// In the case of a left node, there may not be a right sibling.
    /// When we do that, the hash is generated by hashing with ourselves.
    /// So hash_of_sibling would happen to be "my own hash." The parent
    /// node could be None if this is the root node.
    Left {
        hash_of_sibling: <T::Element as HashableElement>::Hash,
        parent: NodeIndex,
    },
    /// A right node always has a left sibling (an append-only merkle
    /// tree fills from left to right). We keep a link to it
    /// instead of to the parent. The parent can be found through
    /// left.parent.
    Right {
        hash_of_sibling: <T::Element as HashableElement>::Hash,
        left: NodeIndex,
    },
    /// There is only one empty node, and it is the parent of the root
    /// node. We store the empty node at position 0 in the vector.
    Empty,
}

impl<T: MerkleHasher> Clone for InternalNode<T> {
    fn clone(&self) -> Self {
        match self {
            InternalNode::Left {
                hash_of_sibling,
                parent,
            } => InternalNode::Left {
                hash_of_sibling: hash_of_sibling.clone(),
                parent: parent.clone(),
            },
            InternalNode::Right {
                hash_of_sibling,
                left,
            } => InternalNode::Right {
                hash_of_sibling: hash_of_sibling.clone(),
                left: left.clone(),
            },
            InternalNode::Empty => InternalNode::Empty,
        }
    }
}

#[derive(Debug, PartialEq)]
struct LeafNode<T: MerkleHasher> {
    element: T::Element,
    parent: NodeIndex,
}

impl<T: MerkleHasher> LeafNode<T> {
    fn new(element: T::Element, parent: NodeIndex) -> Self {
        LeafNode { element, parent }
    }
    fn merkle_hash(&self) -> <T::Element as HashableElement>::Hash {
        self.element.merkle_hash()
    }
}

pub struct LinkedMerkleTree<T: MerkleHasher> {
    hasher: Arc<T>,
    leaves: Vec<LeafNode<T>>,
    nodes: Vec<InternalNode<T>>,
    tree_depth: usize,
}

impl<T: MerkleHasher> LinkedMerkleTree<T> {
    /// The MerkleTree trait has a new associated function that does not
    /// specify the depth. This function is used to make shallower unit tests
    /// that are easier to reason about and faster to execute.
    fn new_with_size(hasher: Arc<T>, tree_depth: usize) -> Box<Self> {
        Box::new(LinkedMerkleTree {
            leaves: vec![],
            nodes: vec![InternalNode::Empty],
            tree_depth: tree_depth - 1,
            hasher,
        })
    }

    /// Get a COPY of the node at a given index. This may panic if the index
    /// is out of bounds. So don't do that (it's a private method,
    /// so an index out of bounds is a coding error).
    ///
    /// Updating the returned value does not change the node on the chain
    ///
    /// This returns a copy to avoid mucking around with mutable
    /// references. In some cases it might be more efficient to use
    /// unsafe code, but I'll let somebody else fix that if profiling
    /// indicates it's an issue.
    fn node_at(&self, index: NodeIndex) -> InternalNode<T> {
        let node_reference = self.nodes.get(index.0 as usize).unwrap();
        (*node_reference).clone()
    }

    fn set_node(&mut self, index: NodeIndex, node: InternalNode<T>) {
        self.nodes[index.0 as usize] = node;
    }

    /// Get the index of the parent of this node. If this is a left node,
    /// the index is the parent. If it's a right node, index is the parent
    /// of the left node.
    fn parent_index(&self, index: NodeIndex) -> NodeIndex {
        match self.node_at(index) {
            InternalNode::Left { parent, .. } => parent,
            InternalNode::Right { left, .. } => self.parent_index(left),
            InternalNode::Empty => NodeIndex::empty(),
        }
    }

    /// Get the parent of the node at the given index.
    fn parent(&self, index: NodeIndex) -> InternalNode<T> {
        let parent_index = self.parent_index(index);
        self.node_at(parent_index)
    }

    /// recalculate all the hashes between the most
    /// recently added leaf in the group
    fn rehash_right_path(&mut self) {
        let mut depth = 0;
        let leaf_index = self.leaves.len() - 1;
        let mut parent_index = self.leaves[leaf_index].parent;
        let mut parent_hash = if is_right_leaf(leaf_index) {
            self.hasher.combine_hash(
                depth,
                &self.leaves[leaf_index - 1].merkle_hash(),
                &self.leaves[leaf_index].merkle_hash(),
            )
        } else {
            self.hasher.combine_hash(
                depth,
                &self.leaves[leaf_index].merkle_hash(),
                &self.leaves[leaf_index].merkle_hash(),
            )
        };
        loop {
            let node = self.node_at(parent_index);
            depth += 1;

            match node {
                InternalNode::Empty => break,
                InternalNode::Left {
                    parent,
                    hash_of_sibling,
                } => {
                    // since we are walking the rightmost path, left nodes do not have
                    // right children. Therefore its sibling hash can be set to
                    // its own hash in its parent half will be set to the combination of
                    // that hash with itself
                    self.set_node(
                        parent_index,
                        InternalNode::Left {
                            parent,
                            hash_of_sibling: parent_hash.clone(),
                        },
                    );
                    parent_index = parent;
                    parent_hash = self.hasher.combine_hash(depth, &parent_hash, &parent_hash);
                }
                InternalNode::Right {
                    left,
                    hash_of_sibling,
                } => {
                    // since this is a new right node we know that we have the correct hash
                    // because we set it correctly when we inserted it. But our left node
                    // needs to have its hash_of_sibling set to our current hash.
                    parent_index = self.parent_index(left);
                    self.set_node(
                        left,
                        InternalNode::Left {
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

    fn most_recent_node_index(&self) -> NodeIndex {
        NodeIndex(self.nodes.len() as u32 - 1)
    }

    // ---------------------------------------------------------------
    // THE BELOW WILL NEED TO MOVE TO impl MerkleTree when I add it
    fn new(hasher: Arc<T>) -> Box<Self> {
        LinkedMerkleTree::new_with_size(hasher, 33)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the number of leaf nodes in the tree
    fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Get the leaf note at a specific position
    fn get(&self, position: usize) -> Option<&<T as MerkleHasher>::Element> {
        self.leaves.get(position).map(|leaf| &leaf.element)
    }

    /// Iterate over clones of all leaf notes in the tree, without consuming
    /// the tree.
    fn iter_notes<'a>(&'a self) -> Box<dyn Iterator<Item = <T as MerkleHasher>::Element> + 'a> {
        Box::new(self.leaves.iter().map(|leaf| leaf.element.clone()))
    }

    /// The current root hash of the tree. Start with the left-most node
    /// and expected depth and walk the tree up to the root.
    fn root_hash(&self) -> Option<<T::Element as HashableElement>::Hash> {
        if self.is_empty() {
            return None;
        }
        let left_hash = self.leaves[0].merkle_hash();
        let right_hash = self
            .leaves
            .get(1)
            .map(|node| node.merkle_hash())
            .or(Some(left_hash.clone()))
            .unwrap();
        let mut depth = 0;
        let mut current_hash = self.hasher.combine_hash(depth, &left_hash, &right_hash);
        let mut current_node_index = self.leaves[0].parent;
        depth = 1;
        while depth != self.tree_depth {
            let current_node = self.node_at(current_node_index);
            current_hash = match current_node {
                InternalNode::Left {
                    hash_of_sibling, ..
                } => self
                    .hasher
                    .combine_hash(depth, &current_hash, &hash_of_sibling),
                InternalNode::Right {
                    hash_of_sibling, ..
                } => self
                    .hasher
                    .combine_hash(depth, &hash_of_sibling, &current_hash),
                InternalNode::Empty => {
                    self.hasher
                        .combine_hash(depth, &current_hash, &current_hash)
                }
            };
            current_node_index = self.parent_index(current_node_index);
            depth += 1;
        }
        Some(current_hash)
    }

    fn past_root(&self, past_size: usize) -> Option<<T::Element as HashableElement>::Hash> {
        let root_depth = depth_at_leaf_count(past_size);
        if self.is_empty() || past_size > self.len() {
            return None;
        }
        let leaf_index = past_size - 1;
        let mut current_hash;
        let mut current_node_index = self.leaves[leaf_index].parent;
        if is_right_leaf(leaf_index) {
            current_hash = self.hasher.combine_hash(
                0,
                &self.leaves[leaf_index - 1].element.merkle_hash(),
                &self.leaves[leaf_index].element.merkle_hash(),
            );
        } else {
            current_hash = self.hasher.combine_hash(
                0,
                &self.leaves[leaf_index].element.merkle_hash(),
                &self.leaves[leaf_index].element.merkle_hash(),
            )
        }

        for depth in 1..std::cmp::min(root_depth, self.tree_depth) {
            match self.node_at(current_node_index) {
                InternalNode::Empty => panic!("depth should not reach empty node"),
                InternalNode::Left {
                    parent,
                    hash_of_sibling,
                } => {
                    current_hash = self
                        .hasher
                        .combine_hash(depth, &current_hash, &current_hash);
                    current_node_index = parent;
                }
                InternalNode::Right {
                    left,
                    hash_of_sibling,
                } => {
                    current_hash = self
                        .hasher
                        .combine_hash(depth, &hash_of_sibling, &current_hash);
                    current_node_index = self.parent_index(left);
                }
            }
        }
        for depth in root_depth..self.tree_depth {
            current_hash = self
                .hasher
                .combine_hash(depth, &current_hash, &current_hash);
        }
        Some(current_hash)
    }

    /// Add a new element to the Merkle Tree, keeping all the hashes
    /// consistent.
    fn add(&mut self, element: T::Element) {
        let hasher = self.hasher.clone();
        let index_of_new_leaf = self.leaves.len();
        if self.leaves.len() >= 2usize.pow(self.tree_depth as u32) {
            panic!("Tree is full");
        }
        let new_parent_index = if self.is_empty() {
            NodeIndex::empty()
        } else {
            NodeIndex::from(self.nodes.len())
        };

        let leaf_hash = element.merkle_hash();
        let leaf = LeafNode {
            element,
            parent: new_parent_index,
        };
        self.leaves.push(leaf);

        if self.leaves.len() == 1 {
            return;
        } else if is_right_leaf(index_of_new_leaf) {
            let left_leaf = &self.leaves[index_of_new_leaf - 1];
            let parent = self.node_at(left_leaf.parent);

            match parent {
                InternalNode::Empty => {
                    let new_parent_of_both = InternalNode::Left {
                        parent: NodeIndex::empty(),
                        hash_of_sibling: self.hasher.combine_hash(
                            0,
                            &left_leaf.element.merkle_hash(),
                            &leaf_hash,
                        ),
                    };
                    self.nodes.push(new_parent_of_both);
                    self.leaves[index_of_new_leaf - 1].parent = new_parent_index;
                }
                _ => {
                    self.leaves[index_of_new_leaf].parent =
                        self.leaves[index_of_new_leaf - 1].parent
                }
            }
        } else {
            // Walk up the path from the previous leaf until find empty or right-hand leaf.
            // Create a bunch of left-hand leaves for each step up that path.
            let mut previous_parent_index = self.leaves[index_of_new_leaf - 1].parent;
            let mut my_hash = hasher.combine_hash(0, &leaf_hash, &leaf_hash);
            let mut depth = 1;
            loop {
                let previous_parent = self.node_at(previous_parent_index);
                match previous_parent {
                    InternalNode::Left {
                        hash_of_sibling,
                        parent,
                    } => {
                        let new_node = InternalNode::Right {
                            left: previous_parent_index,
                            hash_of_sibling: hash_of_sibling.clone(),
                        };
                        self.nodes.push(new_node);
                        if parent == NodeIndex::empty() {
                            let new_parent = InternalNode::Left {
                                parent: NodeIndex::empty(),
                                hash_of_sibling: hasher.combine_hash(
                                    depth,
                                    &hash_of_sibling,
                                    &my_hash,
                                ),
                            };
                            self.nodes.push(new_parent);
                            self.set_node(
                                previous_parent_index,
                                InternalNode::Left {
                                    hash_of_sibling,
                                    parent: self.most_recent_node_index(),
                                },
                            );
                        }
                        break;
                    }
                    InternalNode::Right {
                        hash_of_sibling,
                        left,
                    } => {
                        my_hash = hasher.combine_hash(depth, &my_hash, &my_hash);
                        let new_node = InternalNode::Left {
                            parent: NodeIndex::from(self.nodes.len() + 1), // This is where the next node *WILL* go
                            hash_of_sibling: my_hash.clone(),
                        };
                        self.nodes.push(new_node);
                        previous_parent_index = self.parent_index(left);
                        depth += 1;
                    }
                    InternalNode::Empty => unimplemented!("Empty needs to be handled somehow"),
                }
            }
        }
        self.rehash_right_path();
    }

    /// Truncate the tree when it was a specific past size.
    ///
    /// When we truncate, there may be nodes at the "top"
    /// of the tree that should be removed and replaced with a
    /// link to the empty node. We find this by blocking the rightmost
    /// path from the new leaf. All node index higher than the
    /// the maximum index in that path can be cleared and the leftmost node
    /// habits parent updated to empty.
    fn truncate(&mut self, past_size: usize) {
        if past_size >= self.len() {
            return;
        }
        for _ in 0..self.len() - past_size {
            self.leaves.pop();
        }
        if past_size == 1 {
            self.leaves[0].parent = NodeIndex::empty();
        }
        if past_size == 0 || past_size == 1 {
            self.nodes.clear();
            self.nodes.push(InternalNode::Empty);
            return;
        }
        if past_size == 1 {
            self.leaves.clear();
        }

        let depth = depth_at_leaf_count(self.len()) - 2;
        let mut parent = self.leaves[self.len() - 1].parent;
        let mut max_parent = parent;
        for _ in 0..depth {
            parent = self.parent_index(parent);
            if parent > max_parent {
                max_parent = parent
            }
        }
        match self.node_at(parent) {
            InternalNode::Left {
                hash_of_sibling, ..
            } => {
                self.nodes[parent.0 as usize] = InternalNode::Left {
                    hash_of_sibling,
                    parent: NodeIndex::empty(),
                }
            }
            _ => panic!("new group should be left node"),
        }
        let num_to_remove = self.nodes.len() - max_parent.0 as usize - 1;
        for _ in 0..num_to_remove {
            self.nodes.pop();
        }
        self.rehash_right_path();
    }
}

fn is_right_leaf(value: usize) -> bool {
    value % 2 == 1
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
