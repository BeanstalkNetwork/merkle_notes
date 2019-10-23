use super::{InternalNode, LeafNode, LinkedMerkleTree, NodeIndex};
use crate::test_helper::{CountHasher, StringHasher};

fn leaf(value: char, parent: u32) -> LeafNode<StringHasher> {
    LeafNode {
        element: value.to_string(),
        parent: NodeIndex(parent),
    }
}

fn make_full_tree() -> Box<LinkedMerkleTree<StringHasher>> {
    make_tree("abcdefghijklmnop")
}

fn make_tree(characters: &str) -> Box<LinkedMerkleTree<StringHasher>> {
    let mut tree = LinkedMerkleTree::new(StringHasher::new());
    for character in characters.chars() {
        tree.add(character.to_string());
    }
    tree
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

fn assert_tree(tree: &LinkedMerkleTree<StringHasher>, characters: &str) {
    let expected = make_tree(characters);
    assert_eq!(tree.leaves, expected.leaves);
    assert_eq!(tree.nodes, expected.nodes);
}

#[test]
fn add() {
    color_backtrace::install();
    let mut tree = LinkedMerkleTree::new(StringHasher::new());
    assert_matches!(tree.node_at(NodeIndex(0)), InternalNode::Empty);
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
fn len() {
    let mut tree = LinkedMerkleTree::new(StringHasher::new());
    for i in 0..100 {
        assert_eq!(tree.len(), i);
        tree.add("a".to_string());
    }
}

#[test]
fn iteration_and_get() {
    color_backtrace::install();
    let mut tree = LinkedMerkleTree::new(StringHasher::new());
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
fn truncate() {
    color_backtrace::install();
    let mut tree = LinkedMerkleTree::new(StringHasher::new());
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
fn root_hash_functions() {
    let mut tree = LinkedMerkleTree::new_with_size(StringHasher::new(), 5);
    assert_eq!(tree.root_hash(), None);
    tree.add("a".into());
    assert_eq!(
        tree.root_hash(), Some( "<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>" .to_string() )
    );
    tree.add("b".into());
    assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
    tree.add("c".to_string());
    assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>|<<<a|b-0>|<c|c-0>-1>|<<a|b-0>|<c|c-0>-1>-2>-3>".to_string()));
    tree.add("d".to_string());
    assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<a|b-0>|<c|d-0>-1>-2>-3>".to_string()));
    for i in 0..12 {
        tree.add(i.to_string());
    }
    assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|d-0>-1>|<<0|1-0>|<2|3-0>-1>-2>|<<<4|5-0>|<6|7-0>-1>|<<8|9-0>|<10|11-0>-1>-2>-3>".to_string()));
    assert!(false, " this test is not finished");
}
