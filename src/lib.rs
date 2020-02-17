#[macro_use]
extern crate shrinkwraprs;

use std::fmt::Debug;
use std::io;
use std::sync::Arc;

pub mod linked;
#[cfg(feature = "rocker")]
pub mod rocks;
#[cfg(feature = "sledder")]
pub mod sled;
pub mod vector;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

#[cfg(test)]
pub(crate) mod test_helper;

/// An object that can be used as a hash in a Merkle tree. Basic usage might
/// use bytes or a string here, but in a production system it might be a
/// point on an elliptic curve.
///
/// Any clonable element can be used as a MerkleHash without an adapter (for now)
pub trait MerkleHash: Clone + PartialEq + Debug {}

impl<T> MerkleHash for T where T: Clone + PartialEq + Debug {}

/// A leaf node in the Merkle tree. Each leaf must have the ability to hash
/// itself. The associated combine_hash method is used create a parent hash
/// from two child hashes.
///
/// I made the associated functions operate on this class instead of demanding
/// that such functions exist on the MerkleHash class so that client libraries
/// can use arbitrary third-party types (so long as they are clonable) as hashes.
pub trait HashableElement: Clone + PartialEq + Debug {
    type Hash: MerkleHash;

    /// Calculate the hash of this element
    fn merkle_hash(&self) -> Self::Hash;

    /// Write this element to a writer.
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;
}

/// A factory for working with HashableElements. Specifically, it can read an
/// element from a reader stream, and it can hash two elements together.
///
/// Having this as a separate trait makes it possible to initialize an
/// implementing struct with state that is needed for reading or hashing.
///
/// (In sapling-crypto, this would be the params for a jubjub curve)
pub trait MerkleHasher {
    type Element: HashableElement;

    /// Read an element from a reader.
    fn read_element<R: io::Read>(&self, reader: &mut R) -> io::Result<Self::Element>;

    /// Read a hash from a reader.
    fn read_hash<R: io::Read>(
        &self,
        reader: &mut R,
    ) -> io::Result<<Self::Element as HashableElement>::Hash>;

    /// Write a hash to the writer.
    // In an ideal world, this would live on
    // the MerkleHash trait. However, in Beanstalk's real world, MerkleHash
    // is implemented by a trait in a crate we don't control. Instead of going
    // to the trouble of wrapping it with the NewType pattern, I just add this
    // bit of inelegance here...
    fn write_hash<W: io::Write>(
        &self,
        hash: &<Self::Element as HashableElement>::Hash,
        writer: &mut W,
    ) -> io::Result<()>;

    /// Write a hash to a writer

    /// Hash two child hashes together to calculate the hash of the
    /// new parent.
    ///
    /// Depth is the "level" of the nodes within the tree, where the depth when
    /// hashing two leaves together is zero, when hashing the parents of leaves
    /// it is 1, and so on.
    fn combine_hash(
        &self,
        depth: usize,
        left: &<Self::Element as HashableElement>::Hash,
        right: &<Self::Element as HashableElement>::Hash,
    ) -> <Self::Element as HashableElement>::Hash;
}

/// Interface for an append-only Merkle tree. The methods it supports are
/// specifically useful for crypto-currency style commitments, where each leaf
/// represents one note. There may be other use cases, however.
pub trait MerkleTree {
    type Hasher: MerkleHasher;

    /// Deserialize the Merkle tree from a reader.
    fn read<R: io::Read>(hasher: Arc<Self::Hasher>, reader: &mut R) -> io::Result<Box<Self>>;

    /// Expose the hasher for other APIs to use. Returns an Rc to avoid getting
    /// references clogged up.
    fn hasher(&self) -> Arc<Self::Hasher>;

    /// Insert the new leaf element into the tree, and update all hashes.
    fn add(&mut self, element: <Self::Hasher as MerkleHasher>::Element);

    /// Get a clone of the element at position.
    fn get(&self, position: usize) -> Option<<Self::Hasher as MerkleHasher>::Element>;

    /// Get the number of leaf nodes in the tree.
    fn len(&self) -> usize;

    /// Truncate the tree to the values it contained when it contained past_size
    /// elements.
    ///
    /// After calling, it will contain at most past_size elements, but truncating
    /// to a size that is higher than self.len() is a no-op.
    fn truncate(&mut self, past_size: usize);

    /// Iterate over clones of all leaf notes in the tree, without consuming
    /// the tree.
    fn iter_notes<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = <Self::Hasher as MerkleHasher>::Element> + 'a>;

    /// Get the hash of the current root element in the tree.
    fn root_hash(
        &self,
    ) -> Option<<<Self::Hasher as MerkleHasher>::Element as HashableElement>::Hash>;

    /// Calculate what the root hash was at the time the tree contained
    /// `past_size` elements.
    fn past_root(
        &self,
        past_size: usize,
    ) -> Option<<<Self::Hasher as MerkleHasher>::Element as HashableElement>::Hash>;

    /// Determine whether a tree contained a value in the past, when it had a specific size.
    fn contained(&self, value: &<Self::Hasher as MerkleHasher>::Element, past_size: usize) -> bool;

    /// Determine whether a tree contains a value at its current size.
    fn contains(&self, value: &<Self::Hasher as MerkleHasher>::Element) -> bool {
        self.contained(value, self.len())
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
    fn witness(&self, position: usize) -> Option<Witness<Self::Hasher>>;

    /// Serialize the Merkle tree to a writer.
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;
}

/// Witness to a specific node in an authentication path.
///
/// The Left/Right is the Hash of THIS node, but the MerkleHash at node.0 is
/// the hash of the SIBLING node.
#[derive(PartialEq, Debug)]
pub enum WitnessNode<H: MerkleHash> {
    Left(H),
    Right(H),
}

/// Commitment that a leaf node exists in the tree, with an authentication path
/// and the root_hash of the tree at the time the authentication_path was
/// calculated.
#[derive(PartialEq, Debug)]
pub struct Witness<H: MerkleHasher> {
    pub tree_size: usize,
    pub root_hash: <H::Element as HashableElement>::Hash,
    pub auth_path: Vec<WitnessNode<<H::Element as HashableElement>::Hash>>,
}

impl<H: MerkleHasher> Witness<H> {
    /// verify that the root hash and authentication path on this witness is a
    /// valid confirmation that the given element exists at this point in the
    /// tree.
    pub fn verify(&self, hasher: &H, my_hash: &<H::Element as HashableElement>::Hash) -> bool {
        let mut cur_hash = (*my_hash).clone();
        for (i, node) in self.auth_path.iter().enumerate() {
            cur_hash = match node {
                WitnessNode::Left(ref right_hash) => hasher.combine_hash(i, &cur_hash, right_hash),
                WitnessNode::Right(ref left_hash) => hasher.combine_hash(i, left_hash, &cur_hash),
            }
        }

        cur_hash == self.root_hash
    }
}
