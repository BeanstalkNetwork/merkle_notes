use super::{InternalNode, LeafNode, LinkedMerkleTree, NodeIndex};
use crate::test_helper::{CountHasher, StringHasher};
use crate::{HashableElement, MerkleHasher, MerkleTree, WitnessNode};

fn leaf(value: &str, parent: u32) -> LeafNode<StringHasher> {
    LeafNode {
        element: value.to_string(),
        parent: NodeIndex(parent),
    }
}

#[test]
fn add() {
    color_backtrace::install();
    let mut tree = LinkedMerkleTree::new(StringHasher::new());
    tree.add("a".to_string());
    assert_eq!(tree.leaves.len(), 1);
    assert_eq!(tree.leaves[0], leaf("a", 0));
    assert_eq!(tree.nodes.len(), 1);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    tree.add("b".to_string());
    assert_eq!(tree.leaves.len(), 2);
    assert_eq!(tree.leaves[0], leaf("a", 1));
    assert_eq!(tree.leaves[1], leaf("b", 1));
    assert_eq!(tree.nodes.len(), 2);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert_matches!(
        tree.node_at(NodeIndex(1)),
        InternalNode::Left {
            ref parent, ref hash_of_sibling }
            if parent == &NodeIndex(0) && hash_of_sibling == &"<a|b-0>".to_string()
    );
    tree.add("c".to_string());
    assert_eq!(tree.leaves.len(), 3);
    assert_eq!(tree.leaves[0], leaf("a", 1));
    assert_eq!(tree.leaves[1], leaf("b", 1));
    assert_eq!(tree.leaves[2], leaf("c", 2));
    assert_eq!(tree.nodes.len(), 4);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert_matches!(
        tree.node_at(NodeIndex(1)),
        InternalNode::Left {
            ref parent, ref hash_of_sibling }
            if parent == &NodeIndex(0) && hash_of_sibling == &"<c|c-0>".to_string()
    );
    assert_matches!(
        tree.node_at(NodeIndex(2)),
        InternalNode::Right {
            ref left, ref hash_of_sibling }
            if left == &NodeIndex(1) && hash_of_sibling == &"<a|b-0>".to_string()
    );
    assert_matches!(
        tree.node_at(NodeIndex(3)),
        InternalNode::Left {
            ref parent, ref hash_of_sibling }
            if parent == &NodeIndex(0) && hash_of_sibling == &"<<a|b-0>|<c|c-0>-1>".to_string()
    );
    tree.add("d".to_string());
    assert_eq!(tree.leaves.len(), 4);
}

#[test]
fn root_hash_functions() {
    let mut tree = LinkedMerkleTree::new_with_size(StringHasher::new(), 5);
    assert_eq!(tree.root_hash(), None);
    tree.add("a".into());
    assert_eq!(
        tree.root_hash(), Some( "<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>" .to_string() )
    );
    tree.add("b".into());
    assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
}
