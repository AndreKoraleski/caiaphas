//! Generated code turns the tree a parser builds into its own types.

use std::{ops::Range, str::FromStr, sync::Arc};

use crate::span::{Input, Span};

/// The rules an input matched, each with the rules it matched inside it.
#[derive(Debug)]
pub struct Tree {
    input: Arc<Input>,
    nodes: Vec<NodeData>,
    /// The children of every node, those of each node next to each other.
    children: Vec<usize>,
    root: usize,
}

impl Tree {
    /// Returns the tree of `input` from the node `root`, with the children of
    /// every node of `nodes` in `children`.
    pub(super) fn new(
        input: &str,
        nodes: Vec<NodeData>,
        children: Vec<usize>,
        root: usize,
    ) -> Self {
        Self {
            input: Arc::new(Input::new(input)),
            nodes,
            children,
            root,
        }
    }

    #[must_use]
    pub fn root(&self) -> Node<'_> {
        Node {
            tree: self,
            index: self.root,
        }
    }
}

/// The rule a node matched, where, and where its children are.
#[derive(Debug)]
pub(super) struct NodeData {
    pub(super) rule: u32,
    /// Whether the node stands for its children in the node around it.
    pub(super) inline: bool,
    pub(super) start: usize,
    pub(super) end: usize,
    /// Where the node's children are in the children of its tree.
    pub(super) children: Range<usize>,
}

/// A rule an input matched, in its [`Tree`].
#[derive(Debug, Clone, Copy)]
pub struct Node<'t> {
    tree: &'t Tree,
    index: usize,
}

impl<'t> Node<'t> {
    fn data(&self) -> &'t NodeData {
        &self.tree.nodes[self.index]
    }

    /// Returns the number of the rule this node matched.
    #[must_use]
    pub fn rule(&self) -> usize {
        self.data().rule as usize
    }

    #[must_use]
    pub fn span(&self) -> Span {
        let data = self.data();
        Span::new(Arc::clone(&self.tree.input), data.start..data.end)
    }

    #[must_use]
    pub fn text(&self) -> &'t str {
        let data = self.data();
        &self.tree.input.as_str()[data.start..data.end]
    }

    /// Returns the nodes of the rules matched inside this one, in order.
    pub fn children(&self) -> impl Iterator<Item = Node<'t>> + use<'t> {
        let tree = self.tree;
        tree.children[self.data().children.clone()]
            .iter()
            .map(move |&index| Node { tree, index })
    }

    /// Returns the only rule matched inside this one, if any.
    #[must_use]
    pub fn child(&self) -> Option<Node<'t>> {
        self.children().next()
    }

    /// Returns the rule numbered `rule` matched inside this one. The grammar
    /// makes sure it is there.
    #[must_use]
    pub fn one<T: FromNode>(&self, rule: usize) -> T {
        self.optional(rule)
            .expect("the grammar always matches this rule here")
    }

    /// Returns the rule numbered `rule` matched inside this one, if any.
    pub fn optional<T: FromNode>(&self, rule: usize) -> Option<T> {
        self.children()
            .find(|child| child.rule() == rule)
            .map(T::from_node)
    }

    /// Returns the `N` times the rule numbered `rule` matched inside this one,
    /// in order. The grammar makes sure there are that many.
    #[must_use]
    pub fn array<T: FromNode, const N: usize>(&self, rule: usize) -> [T; N] {
        self.many(rule).try_into().unwrap_or_else(|_| {
            panic!("the grammar always matches this rule {N} times here")
        })
    }

    /// Returns every rule numbered `rule` matched inside this one, in order.
    pub fn many<T: FromNode>(&self, rule: usize) -> Vec<T> {
        self.children()
            .filter(|child| child.rule() == rule)
            .map(T::from_node)
            .collect()
    }
}

/// Builds the type of a rule from the node of its match. Its docs are hidden,
/// since every generated type implements it.
#[doc(hidden)]
pub trait FromNode {
    fn from_node(node: Node<'_>) -> Self;
}

/// The value that the type of a rule reads from the text the rule matched. The
/// parser made sure it reads it.
#[derive(Debug)]
pub struct Converted<T>(pub T);

impl<T: FromStr> FromNode for Converted<T> {
    fn from_node(node: Node<'_>) -> Self {
        match node.text().parse() {
            Ok(value) => Self(value),
            Err(_) => unreachable!("the parser made sure the type reads it"),
        }
    }
}

impl<T: FromNode> FromNode for Box<T> {
    fn from_node(node: Node<'_>) -> Self {
        Box::new(T::from_node(node))
    }
}
