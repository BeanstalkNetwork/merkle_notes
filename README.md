Generic trait for merkle trees. The trait may work for a variety of tree shapes, but for our purposes, a fixed-depth complete binary tree is preferable. 

A merkle tree has leaf nodes that can be hashed. Internal nodes in the tree are hashes that somehow combine the hashes
of its two child nodes.

There is a concrete in-memory implementation of this tree in vector.rs. It's a very stupid implementation designed for our testing purposes. We'll need to create one that is filesystem-aware, has a more efficient use of space, and better memory locality.