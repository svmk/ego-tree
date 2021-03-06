//! Vec-backed mutable tree.
//!
//! `ego_tree` is on [Crates.io][crate] and [GitHub][github].
//!
//! [crate]: https://crates.io/crates/ego-tree
//! [github]: https://github.com/programble/ego-tree
//!
//! # Behaviour
//!
//! - Nodes have zero or more ordered children.
//! - Nodes have at most one parent; orphan nodes are valid.
//! - Individual nodes are not dropped until the tree is dropped.
//! - A node's parent, next sibling, previous sibling, first child and last child can be accessed
//!   in constant time.
//! - Node IDs act as weak references, i.e. they are not tied to the lifetime of the tree.
//!
//! All methods in this crate execute in constant time, and all iterators execute to completion in
//! linear time.
//!
//! # Examples
//!
//! ## Creating a tree
//!
//! ```
//! #[macro_use]
//! extern crate ego_tree;
//!
//! # fn main() {
//! let tree = tree!('a' => { 'b', 'c' => { 'd', 'e' } });
//! # }
//! ```
//!
//! or
//!
//! ```
//! use ego_tree::Tree;
//!
//! let mut tree = Tree::new('a');
//! let mut root = tree.root_mut();
//! root.append('b');
//! let mut c = root.append('c');
//! c.append('d');
//! c.append('e');
//! ```

#![warn(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results,
    variant_size_differences
)]

// Clippy.
#![allow(unknown_lints)]

use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

/// A Vec-backed tree.
///
/// Nodes are allocated in a `Vec` which is only ever pushed to. `NodeId` is an opaque index into
/// the `Vec`.
///
/// Each `Tree` has a unique ID which is also given to each `NodeId` it creates. This is used to
/// bounds check a `NodeId`.
pub struct Tree<T> {
    id: usize,
    vec: Vec<Node<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Node<T> {
    parent: Option<usize>,
    prev_sibling: Option<usize>,
    next_sibling: Option<usize>,
    children: Option<(usize, usize)>,
    value: T,
}

/// A node ID.
///
/// `NodeId` acts as a weak reference which is not tied to the lifetime of the `Tree` that created
/// it.
///
/// With the original `Tree`, a `NodeId` can be used to obtain a `NodeRef` or `NodeMut`.
///
/// # Examples
///
/// ## Obtaining an ID
///
/// ```
/// use ego_tree::Tree;
///
/// let tree = Tree::new('a');
/// let root_id = tree.root().id();
/// ```
///
/// ## Obtaining a `NodeRef`
///
/// ```
/// # use ego_tree::Tree;
/// # let tree = Tree::new('a');
/// # let root_id = tree.root().id();
/// let root = tree.get(root_id);
/// ```
#[derive(Debug)]
pub struct NodeId<T> {
    tree_id: usize,
    index: usize,
    marker: PhantomData<T>,
}

/// A node reference.
#[derive(Debug)]
pub struct NodeRef<'a, T: 'a> {
    tree: &'a Tree<T>,
    node: &'a Node<T>,
    index: usize,
}

/// A node mutator.
#[derive(Debug)]
pub struct NodeMut<'a, T: 'a> {
    tree: &'a mut Tree<T>,
    index: usize,
}

// Implementations.
mod node_id;
mod node_ref;
mod node_mut;
mod debug;

pub mod iter;

// Used to ensure that an Id can only be used with the same Tree that created it.
static TREE_ID_SEQ: AtomicUsize = ATOMIC_USIZE_INIT;
fn tree_id_seq_next() -> usize { TREE_ID_SEQ.fetch_add(1, Ordering::Relaxed) }

impl<T> Node<T> {
    fn new(value: T) -> Self {
        Node {
            parent: None,
            prev_sibling: None,
            next_sibling: None,
            children: None,
            value: value,
        }
    }
}

impl<T> Tree<T> {
    /// Creates a new tree with a root node.
    pub fn new(root: T) -> Self {
        Tree {
            id: tree_id_seq_next(),
            vec: vec![Node::new(root)],
        }
    }

    /// Creates a new tree of the specified capacity with a root node.
    pub fn with_capacity(root: T, capacity: usize) -> Self {
        let mut vec = Vec::with_capacity(capacity);
        vec.push(Node::new(root));
        Tree {
            id: tree_id_seq_next(),
            vec: vec,
        }
    }

    /// Returns a reference to the root node.
    pub fn root(&self) -> NodeRef<T> {
        self.get_unchecked(0)
    }

    /// Returns a mutator of the root node.
    pub fn root_mut(&mut self) -> NodeMut<T> {
        self.get_unchecked_mut(0)
    }

    /// Creates an orphan node, returning a mutator of it.
    pub fn orphan(&mut self, value: T) -> NodeMut<T> {
        let id = self.vec.len();
        self.vec.push(Node::new(value));
        self.get_unchecked_mut(id)
    }

    /// Returns a reference to the specified node.
    ///
    /// # Panics
    ///
    /// Panics if `id` does not refer to a node in this tree.
    pub fn get(&self, id: NodeId<T>) -> NodeRef<T> {
        self.get_unchecked(self.validate_id(id))
    }

    /// Returns a mutator of the specified node.
    ///
    /// # Panics
    ///
    /// Panics if `id` does not refer to a node in this tree.
    pub fn get_mut(&mut self, id: NodeId<T>) -> NodeMut<T> {
        let index = self.validate_id(id);
        self.get_unchecked_mut(index)
    }

    fn validate_id(&self, id: NodeId<T>) -> usize {
        assert_eq!(self.id, id.tree_id);
        id.index
    }

    fn node_id(&self, index: usize) -> NodeId<T> {
        NodeId {
            tree_id: self.id,
            index: index,
            marker: PhantomData,
        }
    }

    fn get_unchecked(&self, index: usize) -> NodeRef<T> {
        NodeRef {
            tree: self,
            node: self.get_node_unchecked(index),
            index: index,
        }
    }

    fn get_unchecked_mut(&mut self, index: usize) -> NodeMut<T> {
        NodeMut {
            tree: self,
            index: index,
        }
    }

    fn get_node_unchecked(&self, index: usize) -> &Node<T> {
        unsafe { self.vec.get_unchecked(index) }
    }

    fn get_node_unchecked_mut(&mut self, index: usize) -> &mut Node<T> {
        unsafe { self.vec.get_unchecked_mut(index) }
    }
}

impl<T: Default> Default for Tree<T> {
    fn default() -> Self {
        Tree::new(T::default())
    }
}

impl<T: Clone> Clone for Tree<T> {
    fn clone(&self) -> Self {
        Tree {
            id: tree_id_seq_next(),
            vec: self.vec.clone(),
        }
    }
}

impl<T: Eq> Eq for Tree<T> { }
impl<T: PartialEq> PartialEq for Tree<T> {
    fn eq(&self, other: &Self) -> bool {
        self.vec == other.vec
    }
}

/// Creates a `Tree` from expressions.
///
/// With no arguments, it is equivalent to `Tree::default`.
///
/// ```
/// #[macro_use]
/// extern crate ego_tree;
/// use ego_tree::Tree;
///
/// # fn main() {
/// let tree: Tree<i32> = tree!();
/// # }
/// ```
///
/// With a single argument, it is equivalent to `Tree::new`.
///
/// ```
/// # #[macro_use]
/// # extern crate ego_tree;
/// # fn main() {
/// let tree = tree!(0i32);
/// # }
/// ```
///
/// With a tree-like argument, a `Tree` is created, mutated, and returned.
///
/// ```
/// # #[macro_use]
/// # extern crate ego_tree;
/// # fn main() {
/// let tree = tree! {
///     "root" => {
///         "child_a",
///         "child_b" => {
///             "grandchild_a",
///             "grandchild_b",
///         },
///         "child_c",
///     }
/// };
/// # }
/// ```
///
/// Note that nodes are inserted in the order they appear, which may not be the most efficient way
/// of constructing the tree.
///
/// Additionally, after inserting the last node, it will travel all the way back up the tree, even
/// when unnecessary.
#[macro_export]
macro_rules! tree {
    (@ $n:ident { }) => { };

    // Last leaf.
    (@ $n:ident { $value:expr }) => {{
        $n.append($value);
    }};

    // Leaf.
    (@ $n:ident { $value:expr, $($tail:tt)* }) => {{
        $n.append($value);
        tree!(@ $n { $($tail)* });
    }};

    // Last node with children.
    (@ $n:ident { $value:expr => $children:tt }) => {{
        let mut node = $n.append($value);
        tree!(@ node $children);
    }};

    // Node with children.
    (@ $n:ident { $value:expr => $children:tt, $($tail:tt)* }) => {{
        {
            let mut node = $n.append($value);
            tree!(@ node $children);
        }
        tree!(@ $n { $($tail)* });
    }};

    () => { $crate::Tree::default() };

    ($root:expr) => { $crate::Tree::new($root) };

    ($root:expr => { }) => { $crate::Tree::new($root) };

    ($root:expr => $children:tt) => {{
        let mut tree = $crate::Tree::new($root);
        {
            let mut node = tree.root_mut();
            tree!(@ node $children);
        }
        tree
    }};
}
