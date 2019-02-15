use std::io;
use std::iter::IntoIterator;

/// Witness to a specific node in an authentication path.
///
/// The Left/Right is the Hash of THIS node, but the HASHTYPE at node.0 is
/// the hash of the SIBLING node.
///
pub enum WitnessNode<HASHTYPE> {
    Left(HASHTYPE),
    Right(HASHTYPE),
}

/// A leaf node in the Merkle tree. Each leaf must have the ability to hash
/// itself. The associated combine_hash method is used create a parent hash
/// from two child hashes.
pub trait HashableElement<HASHTYPE> {
    /// Calculate the hash of this element
    fn merkle_hash(&self) -> HASHTYPE;
    /// Hash two child hashes together to calculate the hash of the
    /// new parent
    fn combine_hash(left: &HASHTYPE, right: &HASHTYPE) -> HASHTYPE;
}

/// Interface for an append-only Merkle tree. The methods it supports are
/// specifically useful for crypto-currency style commitments, where each leaf
/// represents one note. There may be other use cases, however.
pub trait MerkleTree<HASHTYPE, T: HashableElement<HASHTYPE>>: IntoIterator {
    /// Deserialize the Merkle tree from a reader.
    fn read<R: io::Read>(&self, reader: &mut R) -> io::Result<Box<Self>>;

    /// Insert the new leaf element into the tree, and update all hashes.
    fn add(&mut self, element: T);

    /// Get the number of leaf nodes in the tree.
    fn len(&self);

    /// Get the hash of the current root element in the tree.
    fn root_hash(&self) -> HASHTYPE;

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
    fn witness_path(&self, position: usize) -> Vec<WitnessNode<HASHTYPE>>;

    /// Calculate what the root hash was at the time the tree contained
    /// `past_size` elements.
    fn past_root(&self, past_size: usize) -> HASHTYPE;

    /// Serialize the Merkle tree to a writer.
    fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;
}
