//! Generated code turns the tree a parser builds into its own types.

use std::{str::FromStr, sync::Arc};

use super::nodes::{Nodes, Store, Width};
use crate::span::{Input, Span};

/// The rules an input matched, each with the rules it matched inside it.
#[derive(Debug)]
pub struct Tree {
    input: Arc<Input>,
    nodes: Store,
    root: usize,
}

impl Tree {
    /// Returns the tree of `input` from the node `root` of `nodes`.
    pub(super) fn new(input: &str, nodes: Store, root: usize) -> Self {
        Self {
            input: Arc::new(Input::new(input)),
            nodes,
            root,
        }
    }

    /// Returns the value of type `T` that the root of the tree builds. Each
    /// width of node has a `T` built for it, so reading the nodes never asks
    /// which width they have.
    #[must_use]
    pub fn value<T: FromNode>(&self) -> T {
        let input = &self.input;
        let index = self.root;
        match &self.nodes {
            Store::W8(nodes) => T::from_node(Node {
                input,
                nodes,
                index,
            }),
            Store::W16(nodes) => T::from_node(Node {
                input,
                nodes,
                index,
            }),
            Store::W32(nodes) => T::from_node(Node {
                input,
                nodes,
                index,
            }),
            Store::W64(nodes) => T::from_node(Node {
                input,
                nodes,
                index,
            }),
        }
    }
}

/// A rule an input matched, among nodes of width `W`.
#[derive(Debug, Clone, Copy)]
pub struct Node<'t, W> {
    input: &'t Arc<Input>,
    nodes: &'t Nodes<W>,
    index: usize,
}

impl<'t, W: Width> Node<'t, W> {
    /// Returns the number of the rule this node matched.
    #[must_use]
    pub fn rule(&self) -> usize {
        self.nodes.rule(self.index)
    }

    #[must_use]
    pub fn span(&self) -> Span {
        Span::new(Arc::clone(self.input), self.nodes.range(self.index))
    }

    #[must_use]
    pub fn text(&self) -> &'t str {
        &self.input.as_str()[self.nodes.range(self.index)]
    }

    /// Returns the nodes of the rules matched inside this one, in order.
    pub fn children(&self) -> impl Iterator<Item = Node<'t, W>> + use<'t, W> {
        let (input, nodes) = (self.input, self.nodes);
        nodes.children(self.index).iter().map(move |index| Node {
            input,
            nodes,
            index: index.get(),
        })
    }

    /// Returns the only rule matched inside this one, if any.
    #[must_use]
    pub fn child(&self) -> Option<Node<'t, W>> {
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
        let mut found = self
            .children()
            .filter(|child| child.rule() == rule)
            .map(T::from_node);
        std::array::from_fn(|_| {
            found.next().unwrap_or_else(|| {
                panic!("the grammar always matches this rule {N} times here")
            })
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
    fn from_node<W: Width>(node: Node<'_, W>) -> Self;
}

/// The value that the type of a rule reads from the text the rule matched. The
/// parser made sure it reads it.
#[derive(Debug)]
pub struct Converted<T>(pub T);

impl<T: FromStr> FromNode for Converted<T> {
    fn from_node<W: Width>(node: Node<'_, W>) -> Self {
        match node.text().parse() {
            Ok(value) => Self(value),
            Err(_) => unreachable!("the parser made sure the type reads it"),
        }
    }
}

impl<T: FromNode> FromNode for Box<T> {
    fn from_node<W: Width>(node: Node<'_, W>) -> Self {
        Box::new(T::from_node(node))
    }
}
