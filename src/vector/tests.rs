use super::{
    depth_at_index, first_leaf, first_leaf_by_num_leaves, is_complete, is_left_child, parent_index,
    Node, VectorMerkleTree,
};
use crate::test_helper::{CountHasher, StringHasher};
use crate::{HashableElement, MerkleHasher, MerkleTree, WitnessNode};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io;
use std::io::Read;
use std::sync::Arc;

#[test]
fn add() {
    let mut tree = VectorMerkleTree::new(StringHasher::new());
    tree.add("a".to_string());
    assert_eq!(tree.nodes.len(), 1);
    assert_matches!(tree.nodes[0], Node::Leaf(ref e) if *e == "a".to_string());
    tree.add("b".to_string());
    assert_eq!(tree.nodes.len(), 3);
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[1], Node::Leaf(ref e) if *e == "a".to_string());
    assert_matches!(tree.nodes[2], Node::Leaf(ref e) if *e == "b".to_string());
    tree.add("c".to_string());
    assert_eq!(tree.nodes.len(), 6);
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<a|b-0>|<c|c-0>-1>".to_string());
    assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<c|c-0>".to_string());
    assert_matches!(tree.nodes[3], Node::Leaf(ref e) if *e == "a".to_string());
    assert_matches!(tree.nodes[4], Node::Leaf(ref e) if *e == "b".to_string());
    assert_matches!(tree.nodes[5], Node::Leaf(ref e) if *e == "c".to_string());
    tree.add("d".to_string());
    assert_eq!(tree.nodes.len(), 7);
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
    assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
    assert_matches!(tree.nodes[3], Node::Leaf(ref e) if *e == "a".to_string());
    assert_matches!(tree.nodes[4], Node::Leaf(ref e) if *e == "b".to_string());
    assert_matches!(tree.nodes[5], Node::Leaf(ref e) if *e == "c".to_string());
    assert_matches!(tree.nodes[6], Node::Leaf(ref e) if *e == "d".to_string());
    tree.add("e".to_string());
    assert_eq!(tree.nodes.len(), 12);
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|e-0>|<e|e-0>-1>-2>".to_string());
    assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
    assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|e-0>|<e|e-0>-1>".to_string());
    assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
    assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|e-0>".to_string());
    assert_matches!(tree.nodes[6], Node::Empty);
    assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
    assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
    assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
    assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
    assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
    tree.add("f".to_string());
    assert_eq!(tree.nodes.len(), 13);
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<e|f-0>-1>-2>".to_string());
    assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
    assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|f-0>|<e|f-0>-1>".to_string());
    assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
    assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
    assert_matches!(tree.nodes[6], Node::Empty);
    assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
    assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
    assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
    assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
    assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
    assert_matches!(tree.nodes[12], Node::Leaf(ref e) if *e == "f".to_string());
    tree.add("g".to_string());
    assert_eq!(tree.nodes.len(), 14);
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|g-0>-1>-2>".to_string());
    assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
    assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|f-0>|<g|g-0>-1>".to_string());
    assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
    assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
    assert_matches!(tree.nodes[6], Node::Internal(ref e) if *e == "<g|g-0>".to_string());
    assert_matches!(tree.nodes[7], Node::Leaf(ref e) if *e == "a".to_string());
    assert_matches!(tree.nodes[8], Node::Leaf(ref e) if *e == "b".to_string());
    assert_matches!(tree.nodes[9], Node::Leaf(ref e) if *e == "c".to_string());
    assert_matches!(tree.nodes[10], Node::Leaf(ref e) if *e == "d".to_string());
    assert_matches!(tree.nodes[11], Node::Leaf(ref e) if *e == "e".to_string());
    assert_matches!(tree.nodes[12], Node::Leaf(ref e) if *e == "f".to_string());
    assert_matches!(tree.nodes[13], Node::Leaf(ref e) if *e == "g".to_string());
    tree.add("h".to_string());
    assert_eq!(tree.nodes.len(), 15);
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>".to_string());
    assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
    assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<e|f-0>|<g|h-0>-1>".to_string());
    assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
    assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
    assert_matches!(tree.nodes[6], Node::Internal(ref e) if *e == "<g|h-0>".to_string());
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
    assert_matches!(tree.nodes[0], Node::Internal(ref e) if *e == "<<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>|<<<i|i-0>|<i|i-0>-1>|<<i|i-0>|<i|i-0>-1>-2>-3>".to_string());
    assert_matches!(tree.nodes[1], Node::Internal(ref e) if *e == "<<<a|b-0>|<c|d-0>-1>|<<e|f-0>|<g|h-0>-1>-2>".to_string());
    assert_matches!(tree.nodes[2], Node::Internal(ref e) if *e == "<<<i|i-0>|<i|i-0>-1>|<<i|i-0>|<i|i-0>-1>-2>".to_string());
    assert_matches!(tree.nodes[3], Node::Internal(ref e) if *e == "<<a|b-0>|<c|d-0>-1>".to_string());
    assert_matches!(tree.nodes[4], Node::Internal(ref e) if *e == "<<e|f-0>|<g|h-0>-1>".to_string());
    assert_matches!(tree.nodes[5], Node::Internal(ref e) if *e == "<<i|i-0>|<i|i-0>-1>".to_string());
    assert_matches!(tree.nodes[6], Node::Empty);
    assert_matches!(tree.nodes[7], Node::Internal(ref e) if *e == "<a|b-0>".to_string());
    assert_matches!(tree.nodes[8], Node::Internal(ref e) if *e == "<c|d-0>".to_string());
    assert_matches!(tree.nodes[9], Node::Internal(ref e) if *e == "<e|f-0>".to_string());
    assert_matches!(tree.nodes[10], Node::Internal(ref e) if *e == "<g|h-0>".to_string());
    assert_matches!(tree.nodes[11], Node::Internal(ref e) if *e == "<i|i-0>".to_string());
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
fn len() {
    let mut tree = VectorMerkleTree::new(StringHasher::new());
    for i in 0..100 {
        assert_eq!(tree.len(), i);
        tree.add("a".to_string());
    }
}

#[test]
fn contained() {
    let mut tree = VectorMerkleTree::new(StringHasher::new());
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
    let mut tree = VectorMerkleTree::new_with_size(StringHasher::new(), 5);
    assert_eq!(tree.root_hash(), None);
    assert_eq!(tree.past_root(1), None);
    tree.add("a".to_string());
    assert_eq!(
            tree.root_hash(),
            Some(
                "<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>"
                    .to_string()
            )
        );
    assert_eq!(tree.past_root(1), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
    assert_eq!(tree.past_root(2), None);
    tree.add("b".to_string());
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
    let hasher = StringHasher::new();
    // Tree with 4 levels (8 leaves) for easier reasoning
    let mut tree = VectorMerkleTree::new_with_size(hasher, 4);
    assert!(tree.witness(0).is_none());

    tree.add("a".to_string());
    assert!(tree.witness(1).is_none());
    let mut expected_root = "<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>";
    let mut witness = tree.witness(0).expect("path exists");
    assert!(witness.verify(&tree.hasher, &"a".to_string()));
    assert!(!witness.verify(&tree.hasher, &"b".to_string()));
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

#[test]
fn test_truncate() {
    let mut tree = VectorMerkleTree::new_with_size(StringHasher::new(), 5);
    tree.truncate(0);
    tree.truncate(1);

    tree.add("a".to_string());
    tree.truncate(1);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.nodes.len(), 1);
    assert_eq!(tree.iter_notes().next(), Some("a".to_string()));
    assert_eq!(tree.root_hash(), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));
    tree.truncate(0);
    assert_eq!(tree.len(), 0);
    assert_eq!(tree.nodes.len(), 0);
    assert!(tree.root_hash().is_none());

    tree.add("a".to_string());
    tree.add("b".to_string());
    tree.truncate(2);
    assert_eq!(tree.len(), 2);
    assert_eq!(tree.nodes.len(), 3);
    assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>|<<<a|b-0>|<a|b-0>-1>|<<a|b-0>|<a|b-0>-1>-2>-3>".to_string()));
    tree.truncate(1);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.iter_notes().next(), Some("a".to_string()));
    assert_eq!(tree.root_hash(), Some("<<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>|<<<a|a-0>|<a|a-0>-1>|<<a|a-0>|<a|a-0>-1>-2>-3>".to_string()));

    tree.add("b".to_string());
    tree.add("c".to_string());
    tree.add("d".to_string());
    tree.add("e".to_string());
    tree.add("f".to_string());
    tree.add("g".to_string());
    tree.add("h".to_string());
    tree.add("i".to_string());
    tree.truncate(5); // abcde
    assert_eq!(tree.len(), 5);
    let mut iter = tree.iter_notes();
    assert_eq!(iter.next(), Some("a".to_string()));
    assert_eq!(iter.next(), Some("b".to_string()));
    assert_eq!(iter.next(), Some("c".to_string()));
    assert_eq!(iter.next(), Some("d".to_string()));
    assert_eq!(iter.next(), Some("e".to_string()));
    assert_eq!(tree.root_hash(), Some("<<<<a|b-0>|<c|d-0>-1>|<<e|e-0>|<e|e-0>-1>-2>|<<<a|b-0>|<c|d-0>-1>|<<e|e-0>|<e|e-0>-1>-2>-3>".to_string()));
}

#[test]
fn iteration_and_get() {
    color_backtrace::install();
    let mut tree = VectorMerkleTree::new(StringHasher::new());
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
fn serialization() {
    let mut tree = VectorMerkleTree::new_with_size(StringHasher::new(), 5);
    for i in 0..12 {
        tree.add(i.to_string());
    }
    let mut bytes = vec![];
    tree.write(&mut bytes)
        .expect("should be able to write bytes.");

    let read_back_tree: Box<VectorMerkleTree<StringHasher>> =
        VectorMerkleTree::read(StringHasher::new(), &mut bytes[..].as_ref())
            .expect("should be able to read bytes.");

    let mut bytes_again = vec![];
    read_back_tree
        .write(&mut bytes_again)
        .expect("should still be able to write bytes.");
    assert_eq!(bytes, bytes_again);
}

#[test]
fn test_depth_at_index() {
    assert_eq!(depth_at_index(0), 1);
    assert_eq!(depth_at_index(1), 2);
    assert_eq!(depth_at_index(2), 2);
    assert_eq!(depth_at_index(3), 3);
    assert_eq!(depth_at_index(4), 3);
    assert_eq!(depth_at_index(5), 3);
    assert_eq!(depth_at_index(6), 3);
    assert_eq!(depth_at_index(7), 4);
    assert_eq!(depth_at_index(8), 4);
    assert_eq!(depth_at_index(9), 4);
    assert_eq!(depth_at_index(10), 4);
    assert_eq!(depth_at_index(11), 4);
    assert_eq!(depth_at_index(12), 4);
    assert_eq!(depth_at_index(13), 4);
    assert_eq!(depth_at_index(14), 4);
    assert_eq!(depth_at_index(15), 5);
    assert_eq!(depth_at_index(16), 5);
    assert_eq!(depth_at_index(30), 5);
    assert_eq!(depth_at_index(31), 6);
    assert_eq!(depth_at_index(62), 6);
    assert_eq!(depth_at_index(63), 7);
    assert_eq!(depth_at_index(127), 8);
}

#[test]
fn test_first_leaf() {
    assert_eq!(first_leaf(1), 0);
    assert_eq!(first_leaf(2), 1);
    assert_eq!(first_leaf(3), 1);
    assert_eq!(first_leaf(4), 3);
    assert_eq!(first_leaf(5), 3);
    assert_eq!(first_leaf(6), 3);
    assert_eq!(first_leaf(7), 3);
    assert_eq!(first_leaf(8), 7);
    assert_eq!(first_leaf(9), 7);
    assert_eq!(first_leaf(10), 7);
    assert_eq!(first_leaf(11), 7);
    assert_eq!(first_leaf(12), 7);
    assert_eq!(first_leaf(13), 7);
    assert_eq!(first_leaf(14), 7);
    assert_eq!(first_leaf(15), 7);
    assert_eq!(first_leaf(16), 15);
    assert_eq!(first_leaf(31), 15);
    assert_eq!(first_leaf(63), 31);
    assert_eq!(first_leaf(64), 63);
}

#[test]
fn test_first_leaf_by_num_leaves() {
    for i in 1..18 {
        println!(
            "{} {} {}",
            i,
            depth_at_index(i),
            first_leaf_by_num_leaves(i)
        );
    }
    assert_eq!(first_leaf_by_num_leaves(1), 0);
    assert_eq!(first_leaf_by_num_leaves(2), 1);
    assert_eq!(first_leaf_by_num_leaves(3), 3);
    assert_eq!(first_leaf_by_num_leaves(4), 3);
    assert_eq!(first_leaf_by_num_leaves(5), 7);
    assert_eq!(first_leaf_by_num_leaves(6), 7);
    assert_eq!(first_leaf_by_num_leaves(7), 7);
    assert_eq!(first_leaf_by_num_leaves(8), 7);
    assert_eq!(first_leaf_by_num_leaves(9), 15);
    assert_eq!(first_leaf_by_num_leaves(10), 15);
    assert_eq!(first_leaf_by_num_leaves(11), 15);
    assert_eq!(first_leaf_by_num_leaves(12), 15);
    assert_eq!(first_leaf_by_num_leaves(13), 15);
    assert_eq!(first_leaf_by_num_leaves(14), 15);
    assert_eq!(first_leaf_by_num_leaves(15), 15);
    assert_eq!(first_leaf_by_num_leaves(16), 15);
    assert_eq!(first_leaf_by_num_leaves(17), 31);
    assert_eq!(first_leaf_by_num_leaves(32), 31);
    assert_eq!(first_leaf_by_num_leaves(33), 63);
    assert_eq!(first_leaf_by_num_leaves(64), 63);
    assert_eq!(first_leaf_by_num_leaves(65), 127);
    assert_eq!(first_leaf_by_num_leaves(128), 127);
}

#[test]
fn private_tree_mathy_methods() {
    let mut num_nodes = 0;
    assert!(is_complete(num_nodes));
    assert_eq!(depth_at_index(num_nodes), 1);
    // no parent_index check, it should panic

    num_nodes = 1;
    assert!(is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 0);
    assert!(is_left_child(num_nodes));

    num_nodes = 2;
    assert!(!is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 0);
    assert!(!is_left_child(num_nodes));

    num_nodes = 3;
    assert!(is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 1);

    num_nodes = 4;
    assert!(!is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 1);

    num_nodes = 5;
    assert!(!is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 2);

    num_nodes = 6;
    assert!(!is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 2);

    num_nodes = 7;
    assert!(is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 3);

    for _ in 0..7 {
        num_nodes += 1;
        assert!(!is_complete(num_nodes));
    }

    num_nodes = 15;
    assert!(is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 7);

    for _ in 0..15 {
        num_nodes += 1;
        assert!(!is_complete(num_nodes));
    }

    num_nodes = 31;
    assert!(is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 15);

    num_nodes = 32;
    assert!(!is_complete(num_nodes));
    assert_eq!(parent_index(num_nodes), 15);
}

#[test]
fn default_authpath_len() {
    let mut tree = VectorMerkleTree::new(CountHasher::new());
    tree.add(1);
    let witness = tree.witness(0).expect("node exists");
    assert_eq!(witness.root_hash, 33);
    assert_eq!(witness.auth_path.len(), 32);
}
