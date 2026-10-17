//! Nodes hold the tree a parser builds. Their positions and indices take the
//! fewest bits that fit the input, so a small input takes little memory. A
//! parser is compiled once for each width, so that matching never asks which
//! width it has.

use std::{fmt::Debug, ops::Range};

/// An unsigned integer that holds the positions and indices of nodes.
pub trait Width: Copy + Debug + 'static {
    /// The largest value of this width, or `usize::MAX` when that is less.
    const MAX: usize;

    /// Returns `value`, which fits, as this width.
    fn cut(value: usize) -> Self;

    fn get(self) -> usize;

    /// Returns `nodes` as nodes of any width.
    fn store(nodes: Nodes<Self>) -> Store;
}

macro_rules! widths {
    ($($width:ty => $variant:ident),*) => {$(
        impl Width for $width {
            // The cast happens only where the width's largest value fits.
            #[allow(clippy::cast_possible_truncation)]
            const MAX: usize =
                if <$width>::MAX as u128 > usize::MAX as u128 {
                    usize::MAX
                } else {
                    <$width>::MAX as usize
                };

            // The callers make sure the value fits.
            #[allow(clippy::cast_possible_truncation)]
            #[inline]
            fn cut(value: usize) -> Self {
                value as Self
            }

            // Every value of a width came from a `usize`.
            #[allow(clippy::cast_possible_truncation)]
            #[inline]
            fn get(self) -> usize {
                self as usize
            }

            fn store(nodes: Nodes<Self>) -> Store {
                Store::$variant(nodes)
            }
        }
    )*};
}

widths!(u8 => W8, u16 => W16, u32 => W32, u64 => W64);

/// The rule a node matched, where, and where its children are.
#[derive(Debug)]
struct Data<W> {
    rule: u32,
    /// Whether the node stands for its children in the node around it.
    inline: bool,
    start: W,
    end: W,
    /// Where the node's children start and end in the children of all nodes.
    first: W,
    last: W,
}

/// The nodes a parser builds, of one width.
#[derive(Debug)]
pub struct Nodes<W> {
    data: Vec<Data<W>>,
    /// The children of every node, those of each node next to each other.
    children: Vec<W>,
    /// The nodes matched inside the rules being matched, those of each rule
    /// after those of the rules around it.
    matched: Vec<W>,
}

impl<W: Width> Nodes<W> {
    pub(crate) fn new() -> Self {
        Self {
            data: Vec::new(),
            children: Vec::new(),
            matched: Vec::new(),
        }
    }

    /// Returns how many nodes have been matched and not yet given to a node.
    #[inline]
    pub(crate) fn mark(&self) -> usize {
        self.matched.len()
    }

    /// Forgets the nodes matched since `mark`.
    #[inline]
    pub(crate) fn truncate(&mut self, mark: usize) {
        self.matched.truncate(mark);
    }

    /// Forgets the nodes matched from `mark` to `start`.
    #[inline]
    pub(crate) fn discard(&mut self, mark: usize, start: usize) {
        self.matched.drain(mark..start);
    }

    /// Adds `node`, which these nodes built, to the nodes matched.
    #[inline]
    pub(crate) fn add(&mut self, node: usize) {
        self.matched.push(W::cut(node));
    }

    /// Returns the node of `rule` over `range`, with the nodes matched since
    /// `mark` as its children, or `None` when there are too many nodes for
    /// this width to count, leaving the nodes as they were.
    #[inline]
    pub(crate) fn node(
        &mut self,
        rule: u32,
        inline: bool,
        range: &Range<usize>,
        mark: usize,
    ) -> Option<usize> {
        // A node adds at most one child for each node, and copies at most the
        // children there are, so below a quarter of the width both stay within
        // it.
        if self.data.len() >= W::MAX / 4 || self.children.len() >= W::MAX / 4 {
            return None;
        }
        let index = self.data.len();
        let first = W::cut(self.children.len());
        // A child of an inline rule stands where its node would.
        for child in self.matched.drain(mark..) {
            let data = &self.data[child.get()];
            if data.inline {
                self.children
                    .extend_from_within(data.first.get()..data.last.get());
            } else {
                self.children.push(child);
            }
        }
        let last = W::cut(self.children.len());
        self.data.push(Data {
            rule,
            inline,
            start: W::cut(range.start),
            end: W::cut(range.end),
            first,
            last,
        });
        Some(index)
    }

    /// Returns the number of the rule `node` matched.
    #[inline]
    pub(crate) fn rule(&self, node: usize) -> usize {
        self.data[node].rule as usize
    }

    /// Returns where `node` matched in the input.
    #[inline]
    pub(crate) fn range(&self, node: usize) -> Range<usize> {
        let data = &self.data[node];
        data.start.get()..data.end.get()
    }

    /// Returns the children of `node`, in order.
    #[inline]
    pub(crate) fn children(&self, node: usize) -> &[W] {
        let data = &self.data[node];
        &self.children[data.first.get()..data.last.get()]
    }
}

/// Returns whether an input of `length` bytes fits a parser of width `W`, with
/// room for about as many nodes as it has bytes.
pub(crate) fn fits<W: Width>(length: usize) -> bool {
    length < W::MAX / 4
}

/// The nodes of a tree, of whatever width they were built in.
#[derive(Debug)]
pub enum Store {
    W8(Nodes<u8>),
    W16(Nodes<u16>),
    W32(Nodes<u32>),
    W64(Nodes<u64>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_an_input_in_a_quarter_of_a_width() {
        assert!(fits::<u8>(62));
        assert!(!fits::<u8>(63));
        assert!(fits::<u16>(16_382));
        assert!(fits::<u64>(usize::MAX / 5));
    }

    #[test]
    fn stops_where_the_nodes_outgrow_their_width() {
        let mut nodes = Nodes::<u8>::new();
        let built: Vec<usize> = (0..63)
            .map_while(|_| nodes.node(1, false, &(0..2), nodes.mark()))
            .collect();
        assert_eq!(built.len(), 63);
        assert_eq!(nodes.node(1, false, &(0..2), nodes.mark()), None);
    }

    #[test]
    fn gives_an_inline_node_s_children_to_the_node_around_it() {
        let mut nodes = Nodes::<u16>::new();
        let first = nodes.node(1, false, &(0..1), 0).unwrap();
        let second = nodes.node(1, false, &(1..2), 0).unwrap();
        nodes.add(first);
        nodes.add(second);
        let inline = nodes.node(3, true, &(0..2), 0).unwrap();
        nodes.add(inline);
        let root = nodes.node(2, false, &(0..2), 0).unwrap();
        let children: Vec<usize> = nodes
            .children(root)
            .iter()
            .map(|child| child.get())
            .collect();
        assert_eq!(children, [first, second]);
        assert_eq!((nodes.rule(root), nodes.range(root)), (2, 0..2));
    }
}
