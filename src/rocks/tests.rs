use super::rocker::{LeafIndex, Node, NodeIndex};
use super::RocksMerkleTree;
use crate::{test_helper::StringHasher, MerkleTree, WitnessNode};
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

#[test]
fn len() {
    color_backtrace::install();

    let mut tree = make_tree("");
    for i in 0..100 {
        assert_eq!(tree.len(), i);
        tree.add("a".to_string());
    }
}

#[test]
fn iteration_and_get() {
    color_backtrace::install();
    let mut tree = make_tree("");
    {
        assert_eq!(tree.get(0), None);
        let mut iter = tree.iter_notes();
        assert_eq!(iter.next(), None);
    }

    tree.add("a".to_string());
    {
        assert_eq!(tree.get(1), None);
        assert_eq!(*tree.get(0).unwrap(), "a".to_string());
        let mut iter = tree.iter_notes();
        assert_eq!(iter.next(), Some("a".to_string()));
        assert_eq!(iter.next(), None);
    }

    tree.add("b".to_string());
    {
        assert_eq!(tree.get(2), None);
        assert_eq!(*tree.get(0).unwrap(), "a".to_string());
        assert_eq!(*tree.get(1).unwrap(), "b".to_string());
        let mut iter = tree.iter_notes();
        assert_eq!(iter.next(), Some("a".to_string()));
        assert_eq!(iter.next(), Some("b".to_string()));
        assert_eq!(iter.next(), None);
    }
    tree.add("c".to_string());
    {
        assert_eq!(tree.get(3), None);
        assert_eq!(*tree.get(0).unwrap(), "a".to_string());
        assert_eq!(*tree.get(1).unwrap(), "b".to_string());
        assert_eq!(*tree.get(2).unwrap(), "c".to_string());
        let mut iter = tree.iter_notes();
        assert_eq!(iter.next(), Some("a".to_string()));
        assert_eq!(iter.next(), Some("b".to_string()));
        assert_eq!(iter.next(), Some("c".to_string()));
        assert_eq!(iter.next(), None);
    }
    tree.add("d".to_string());
    {
        assert_eq!(tree.get(4), None);
        assert_eq!(*tree.get(0).unwrap(), "a".to_string());
        assert_eq!(*tree.get(1).unwrap(), "b".to_string());
        assert_eq!(*tree.get(2).unwrap(), "c".to_string());
        assert_eq!(*tree.get(3).unwrap(), "d".to_string());
        let mut iter = tree.iter_notes();
        assert_eq!(iter.next(), Some("a".to_string()));
        assert_eq!(iter.next(), Some("b".to_string()));
        assert_eq!(iter.next(), Some("c".to_string()));
        assert_eq!(iter.next(), Some("d".to_string()));
        assert_eq!(iter.next(), None);
    }

    tree.add("e".to_string());
    {
        assert_eq!(tree.get(5), None);
        assert_eq!(*tree.get(0).unwrap(), "a".to_string());
        assert_eq!(*tree.get(1).unwrap(), "b".to_string());
        assert_eq!(*tree.get(2).unwrap(), "c".to_string());
        assert_eq!(*tree.get(3).unwrap(), "d".to_string());
        assert_eq!(*tree.get(4).unwrap(), "e".to_string());
        let mut iter = tree.iter_notes();
        assert_eq!(iter.next(), Some("a".to_string()));
        assert_eq!(iter.next(), Some("b".to_string()));
        assert_eq!(iter.next(), Some("c".to_string()));
        assert_eq!(iter.next(), Some("d".to_string()));
        assert_eq!(iter.next(), Some("e".to_string()));
        assert_eq!(iter.next(), None);
    }

    tree.add("f".to_string());
    {
        assert_eq!(tree.get(6), None);
        assert_eq!(*tree.get(0).unwrap(), "a".to_string());
        assert_eq!(*tree.get(1).unwrap(), "b".to_string());
        assert_eq!(*tree.get(2).unwrap(), "c".to_string());
        assert_eq!(*tree.get(3).unwrap(), "d".to_string());
        assert_eq!(*tree.get(4).unwrap(), "e".to_string());
        assert_eq!(*tree.get(5).unwrap(), "f".to_string());
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
fn contained() {
    let mut tree = make_tree("");
    assert!(!tree.contained(&1.to_string(), 0));
    assert!(!tree.contained(&1.to_string(), 1));
    for i in 1..20 {
        tree.add(i.to_string());
        assert!(tree.contained(&i.to_string(), i));
        assert!(tree.contained(&i.to_string(), i + 1));
        assert!(!tree.contained(&i.to_string(), i - 1));
        assert!(tree.contains(&i.to_string()));
    }
}

#[test]
fn root_hash_functions() {
    color_backtrace::install();
    let rocks_directory = tempdir().unwrap();
    let mut tree = RocksMerkleTree::new_with_size(StringHasher::new(), rocks_directory.path(), 5);
    assert_eq!(tree.root_hash(), None);
    assert_eq!(tree.past_root(0), None);
    assert_eq!(tree.past_root(1), None);
    tree.add("a".into());
    assert_eq!(
        tree.root_hash(), Some( "<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>" .to_string() )
    );
    assert_eq!(tree.past_root(1), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
    assert_eq!(tree.past_root(2), None);
    tree.add("b".into());
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
    color_backtrace::install();
    let rocks_directory = tempdir().unwrap();
    let mut tree = RocksMerkleTree::new_with_size(StringHasher::new(), rocks_directory.path(), 4);
    assert!(tree.witness(0).is_none());

    tree.add("a".to_string());
    assert!(tree.witness(1).is_none());
    let mut expected_root = "<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>";
    let mut witness = tree.witness(0).expect("path exists");
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
    assert!(witness.verify(&tree.hasher, &"a".to_string()));
    assert!(!witness.verify(&tree.hasher, &"b".to_string()));

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
