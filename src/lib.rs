use std::io;

pub mod vector;

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

/// An object that can be used as a hash in a Merkle tree. Basic usage might
/// use bytes or a string here, but in a production system it might be a
/// point on an elliptic curve.
///
/// Any clonable element can be used as a MerkleHash without an adapter (for now)
pub trait MerkleHash: Clone {}

impl<T> MerkleHash for T where T: Clone {}

/// Witness to a specific node in an authentication path.
///
/// The Left/Right is the Hash of THIS node, but the MerkleHash at node.0 is
/// the hash of the SIBLING node.
pub enum WitnessNode<H: MerkleHash> {
    Left(H),
    Right(H),
}

/// A leaf node in the Merkle tree. Each leaf must have the ability to hash
/// itself. The associated combine_hash method is used create a parent hash
/// from two child hashes.
///
/// I made the associated functions operate on this class instead of demanding
/// that such functions exist on the MerkleHash class so that client libraries
/// can use arbitrary third-party types (so long as they are clonable) as hashes.
pub trait HashableElement: Sized {
    type Hash: MerkleHash;

    /// Calculate the hash of this element
    fn merkle_hash(&self) -> Self::Hash;

    /// Write this element to a writer.
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;

    /// Read an element from a reader.
    fn read<R: io::Read>(reader: &mut R) -> io::Result<Self>;

    /// Hash two child hashes together to calculate the hash of the
    /// new parent
    fn combine_hash(left: &Self::Hash, right: &Self::Hash) -> Self::Hash;
}

/// Interface for an append-only Merkle tree. The methods it supports are
/// specifically useful for crypto-currency style commitments, where each leaf
/// represents one note. There may be other use cases, however.
pub trait MerkleTree
where
    for<'a> &'a Self: IntoIterator,
{
    type Element: HashableElement;
    /// Deserialize the Merkle tree from a reader.
    fn read<R: io::Read>(reader: &mut R) -> io::Result<Box<Self>>;

    /// Insert the new leaf element into the tree, and update all hashes.
    fn add(&mut self, element: Self::Element);

    /// Get the number of leaf nodes in the tree.
    fn len(&self) -> usize;

    /// Get the hash of the current root element in the tree.
    fn root_hash(&self) -> Option<<Self::Element as HashableElement>::Hash>;

    /// Calculate what the root hash was at the time the tree contained
    /// `past_size` elements.
    fn past_root(&self, past_size: usize) -> Option<<Self::Element as HashableElement>::Hash>;

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
    fn witness_path(
        &self,
        position: usize,
    ) -> Option<Vec<WitnessNode<<Self::Element as HashableElement>::Hash>>>;

    /// Serialize the Merkle tree to a writer.
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;
}
