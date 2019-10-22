use super::{InternalNode, LeafNode, LinkedMerkleTree, NodeIndex};
use crate::test_helper::{CountHasher, StringHasher};
use crate::{HashableElement, MerkleHasher, MerkleTree, WitnessNode};

fn leaf(value: char, parent: u32) -> LeafNode<StringHasher> {
    LeafNode {
        element: value.to_string(),
        parent: NodeIndex(parent),
    }
}

fn node_matches(
    tree: &LinkedMerkleTree<StringHasher>,
    my_index: u32,
    is_left: bool,
    other_index: u32, // parent or left, depending
    expected_hash_of_sibling: &str,
) -> bool {
    let node = tree.node_at(NodeIndex(my_index));
    let is_match = match node.clone() {
        InternalNode::Empty => panic!("node_matches not expected on empty node"),
        InternalNode::Left {
            parent,
            hash_of_sibling,
        } => {
            is_left
                && parent == NodeIndex(other_index)
                && hash_of_sibling == expected_hash_of_sibling.to_string()
        }
        InternalNode::Right {
            left,
            hash_of_sibling,
        } => {
            !is_left
                && left == NodeIndex(other_index)
                && hash_of_sibling == expected_hash_of_sibling.to_string()
        }
    };
    if !is_match {
        dbg!(node);
    }
    is_match
}

fn assert_leaves(tree: &LinkedMerkleTree<StringHasher>, characters: &str, parents: &[u32]) {
    assert_eq!(tree.leaves.len(), characters.len());
    assert_eq!(tree.leaves.len(), parents.len());
    for (index, (character, parent)) in characters.chars().zip(parents).enumerate() {
        assert_eq!(tree.leaves[index], leaf(character, *parent));
    }
}

#[test]
fn add() {
    color_backtrace::install();
    let mut tree = LinkedMerkleTree::new(StringHasher::new());
    tree.add("a".to_string());
    assert_leaves(&tree, "a", &[0]);
    assert_eq!(tree.nodes.len(), 1);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    tree.add("b".to_string());
    assert_leaves(&tree, "ab", &[1, 1]);
    assert_eq!(tree.nodes.len(), 2);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 0, "<a|b-0>"));
    tree.add("c".to_string());
    assert_leaves(&tree, "abc", &[1, 1, 2]);
    assert_eq!(tree.nodes.len(), 4);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|c-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 0, "<<a|b-0>|<c|c-0>-1>"));
    tree.add("d".to_string());
    assert_leaves(&tree, "abcd", &[1, 1, 2, 2]);
    assert_eq!(tree.nodes.len(), 4);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|d-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 0, "<<a|b-0>|<c|d-0>-1>"));
    tree.add("e".to_string());
    assert_leaves(&tree, "abcde", &[1, 1, 2, 2, 4]);
    assert_eq!(tree.nodes.len(), 7);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|d-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 6, "<<e|e-0>|<e|e-0>-1>"));
    assert!(node_matches(&tree, 4, true, 5, "<e|e-0>"));
    assert!(node_matches(&tree, 5, false, 3, "<<a|b-0>|<c|d-0>-1>"));
    assert!(node_matches(
        &tree,
        6,
        true,
        0,
        "<<<a|b-0>|<c|d-0>-1>|<<e|e-0>|<e|e-0>-1>-2>"
    ));
    tree.add("f".to_string());
    assert_leaves(&tree, "abcdef", &[1, 1, 2, 2, 4, 4]);
    assert_eq!(tree.nodes.len(), 7);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|d-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 6, "<<e|f-0>|<e|f-0>-1>"));
    assert!(node_matches(&tree, 4, true, 5, "<e|f-0>"));
    assert!(node_matches(&tree, 5, false, 3, "<<a|b-0>|<c|d-0>-1>"));
    assert!(node_matches(
        &tree,
        6,
        true,
        0,
        "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<e|f-0>-1>-2>"
    ));
    tree.add("g".to_string());
    assert_leaves(&tree, "abcdefg", &[1, 1, 2, 2, 4, 4, 7]);
    assert_eq!(tree.nodes.len(), 8);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|d-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 6, "<<e|f-0>|<g|g-0>-1>"));
    assert!(node_matches(&tree, 4, true, 5, "<g|g-0>"));
    assert!(node_matches(&tree, 5, false, 3, "<<a|b-0>|<c|d-0>-1>"));
    assert!(node_matches(
        &tree,
        6,
        true,
        0,
        "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|g-0>-1>-2>"
    ));
    assert!(node_matches(&tree, 7, false, 4, "<e|f-0>"));
    tree.add("h".to_string());
    assert_leaves(&tree, "abcdefgh", &[1, 1, 2, 2, 4, 4, 7, 7]);
    assert_eq!(tree.nodes.len(), 8);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|d-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 6, "<<e|f-0>|<g|h-0>-1>"));
    assert!(node_matches(&tree, 4, true, 5, "<g|h-0>"));
    assert!(node_matches(&tree, 5, false, 3, "<<a|b-0>|<c|d-0>-1>"));
    assert!(node_matches(
        &tree,
        6,
        true,
        0,
        "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>"
    ));
    assert!(node_matches(&tree, 7, false, 4, "<e|f-0>"));
    tree.add("i".to_string());
    assert_leaves(&tree, "abcdefghi", &[1, 1, 2, 2, 4, 4, 7, 7, 8]);
    assert_eq!(tree.nodes.len(), 12);
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|d-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 6, "<<e|f-0>|<g|h-0>-1>"));
    assert!(node_matches(&tree, 4, true, 5, "<g|h-0>"));
    assert!(node_matches(&tree, 5, false, 3, "<<a|b-0>|<c|d-0>-1>"));
    assert!(node_matches(
        &tree,
        6,
        true,
        11,
        "<<<i|i-0>|<i|i-0>-1>|<<i|i-0>|<i|i-0>-1>-2>"
    ));
    assert!(node_matches(&tree, 7, false, 4, "<e|f-0>"));
    assert!(node_matches(&tree, 8, true, 9, "<i|i-0>"));
    assert!(node_matches(&tree, 9, true, 10, "<<i|i-0>|<i|i-0>-1>"));
    assert!(node_matches(
        &tree,
        10,
        false,
        6,
        "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>"
    ));
    assert!(node_matches(&tree, 11, true, 0, "<<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>|<<<i|i-0>|<i|i-0>-1>|<<i|i-0>|<i|i-0>-1>-2>-3>"));
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
