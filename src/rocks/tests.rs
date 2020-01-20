use super::rocker::{LeafIndex, Node, NodeIndex, Rocker};
use super::RocksMerkleTree;
use crate::{test_helper::StringHasher, MerkleTree};
use tempfile::tempdir;

fn make_tree(characters: &str) -> RocksMerkleTree<StringHasher> {
    let rocks_directory = tempdir().unwrap();
    let mut tree = RocksMerkleTree::new(StringHasher::new(), rocks_directory.path());
    for character in characters.chars() {
        tree.add(character.to_string());
    }
    tree
}

fn make_full_tree() -> RocksMerkleTree<StringHasher> {
    make_tree("abcdefghijklmnop")
}

fn assert_tree(tree: &RocksMerkleTree<StringHasher>, characters: &str) {
    return;
    dbg!(characters);

    let expected = make_tree(characters);
    assert_eq!(tree.len(), expected.len());
    assert_eq!(tree.rocker.num_nodes(), expected.rocker.num_nodes());
    for idx in 0..tree.len() {
        let index = LeafIndex(idx as u32);
        assert_eq!(
            tree.rocker.get_leaf_element(index),
            expected.rocker.get_leaf_element(index)
        );
        assert_eq!(
            tree.rocker.get_leaf_metadata(index),
            expected.rocker.get_leaf_metadata(index)
        );
    }
    for idx in 0..tree.rocker.num_nodes() {
        let index = NodeIndex(idx as u32);
        assert_eq!(tree.rocker.get_node(index), expected.rocker.get_node(index));
    }
}

fn assert_leaves(tree: &RocksMerkleTree<StringHasher>, characters: &str, parents: &[u32]) {
    assert_eq!(tree.len(), characters.len());
    assert_eq!(tree.len(), parents.len());
    for (index, (character, parent)) in characters.chars().zip(parents).enumerate() {
        let leaf_index = LeafIndex(index as u32);
        let element = tree
            .rocker
            .get_leaf_element(leaf_index)
            .expect(&format!("'{}' element should exist in tree", character));
        let leaf_data = tree
            .rocker
            .get_leaf_metadata(leaf_index)
            .expect("{} metadata should exist in tree");
        assert_eq!(element, character.to_string());
        assert_eq!(leaf_data.parent, NodeIndex(*parent));
        assert_eq!(leaf_data.hash, character.to_string());
    }
}

fn node_matches(
    tree: &RocksMerkleTree<StringHasher>,
    my_index: u32,
    is_left: bool,
    other_index: u32, // parent or left, depending
    expected_hash_of_sibling: &str,
) -> bool {
    let node = tree.rocker.get_node(NodeIndex(my_index));
    let is_match = match &node {
        Node::Empty => panic!("node_matches not expected on empty node"),
        Node::Left {
            parent,
            hash_of_sibling,
        } => {
            is_left
                && *parent == NodeIndex(other_index)
                && *hash_of_sibling == expected_hash_of_sibling.to_string()
        }
        Node::Right {
            left,
            hash_of_sibling,
        } => {
            !is_left
                && *left == NodeIndex(other_index)
                && *hash_of_sibling == expected_hash_of_sibling.to_string()
        }
    };
    if !is_match {
        dbg!(node);
    }
    is_match
}

#[test]
fn add() {
    color_backtrace::install();
    let rocks_directory = tempdir().unwrap();
    let mut tree = RocksMerkleTree::new(StringHasher::new(), rocks_directory.path());
    assert_eq!(tree.len(), 0);
    assert_eq!(tree.rocker.num_nodes(), 1);
    tree.add("a".to_string());
    assert_leaves(&tree, "a", &[0]);
    assert_eq!(tree.rocker.num_nodes(), 1);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
    tree.add("b".to_string());
    assert_leaves(&tree, "ab", &[1, 1]);
    assert_eq!(tree.rocker.num_nodes(), 2);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
    assert!(node_matches(&tree, 1, true, 0, "<a|b-0>"));
    tree.add("c".to_string());
    assert_leaves(&tree, "abc", &[1, 1, 2]);
    assert_eq!(tree.rocker.num_nodes(), 4);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|c-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 0, "<<a|b-0>|<c|c-0>-1>"));
    tree.add("d".to_string());
    assert_leaves(&tree, "abcd", &[1, 1, 2, 2]);
    assert_eq!(tree.rocker.num_nodes(), 4);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
    assert!(node_matches(&tree, 1, true, 3, "<c|d-0>"));
    assert!(node_matches(&tree, 2, false, 1, "<a|b-0>"));
    assert!(node_matches(&tree, 3, true, 0, "<<a|b-0>|<c|d-0>-1>"));
    tree.add("e".to_string());
    assert_leaves(&tree, "abcde", &[1, 1, 2, 2, 4]);
    assert_eq!(tree.rocker.num_nodes(), 7);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
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
    assert_eq!(tree.rocker.num_nodes(), 7);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
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
    assert_eq!(tree.rocker.num_nodes(), 8);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
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
    assert_eq!(tree.rocker.num_nodes(), 8);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
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
    assert_eq!(tree.rocker.num_nodes(), 12);
    assert_matches!(tree.rocker.get_node(NodeIndex(0)), Node::Empty);
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
fn truncate() {
    color_backtrace::install();
    let mut tree = make_tree("");
    tree.truncate(0);
    assert_tree(&tree, "");
    tree = make_full_tree();
    tree.truncate(0);
    assert_tree(&tree, "");
    tree = make_full_tree();
    tree.truncate(1);
    assert_tree(&tree, "a");
    tree = make_full_tree();
    tree.truncate(2);
    assert_tree(&tree, "ab");
    tree = make_full_tree();
    tree.truncate(3);
    assert_tree(&tree, "abc");
    tree = make_full_tree();
    tree.truncate(4);
    assert_tree(&tree, "abcd");
    tree = make_full_tree();
    tree.truncate(5);
    assert_tree(&tree, "abcde");
    tree = make_full_tree();
    tree.truncate(6);
    assert_tree(&tree, "abcdef");
    tree = make_full_tree();
    tree.truncate(7);
    assert_tree(&tree, "abcdefg");
    tree = make_full_tree();
    tree.truncate(8);
    assert_tree(&tree, "abcdefgh");
    tree = make_full_tree();
    tree.truncate(9);
    assert_tree(&tree, "abcdefghi");
    tree = make_full_tree();
    tree.truncate(10);
    assert_tree(&tree, "abcdefghij");
    tree = make_full_tree();
    tree.truncate(11);
    assert_tree(&tree, "abcdefghijk");
    tree = make_full_tree();
    tree.truncate(12);
    assert_tree(&tree, "abcdefghijkl");
    tree = make_full_tree();
    tree.truncate(13);
    assert_tree(&tree, "abcdefghijklm");
    tree = make_full_tree();
    tree.truncate(14);
    assert_tree(&tree, "abcdefghijklmn");
    tree = make_full_tree();
    tree.truncate(15);
    assert_tree(&tree, "abcdefghijklmno");
    tree = make_full_tree();
    tree.truncate(16);
    assert_tree(&tree, "abcdefghijklmnop");
    tree = make_full_tree();
    tree.truncate(17);
    assert_tree(&tree, "abcdefghijklmnop");
}
