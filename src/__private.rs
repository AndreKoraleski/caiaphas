//! What generated parsers are made of. Generated code describes each rule of
//! its grammar with these tables, and turns the tree the engine builds into
//! its own types. Nothing here is meant to be used directly, so it can change
//! in any release.
//!
//! Each choice takes the branch that matches the most text, and the first
//! written of those that tie. Optional and repeated sequences take as much as
//! they can. A rule that a parser can try more than once at the same place
//! remembers its match there, so trying every branch never matches it twice.
//!
//! A parse notes nothing about why an input might not match. Only when it does
//! not match is it matched again, noting at each failure what could have been
//! read, to say why.

use std::{ops::Range, sync::Arc};

use rustc_hash::FxHashMap;

use crate::{
    error::{Error, Expected},
    span::{Input, Span},
};

/// The rules of a grammar, indexed by the numbers that refer to them.
#[derive(Debug)]
pub struct Grammar {
    pub rules: &'static [Rule],
}

/// A rule of a [`Grammar`].
#[derive(Debug)]
pub struct Rule {
    /// The name of the rule as written, which errors use for it.
    pub name: &'static str,
    pub expression: Expression,
    /// Whether the rule is left out of the tree and of errors, along with what
    /// it matches.
    pub hidden: bool,
    /// Whether the rule remembers its match at each position, which only a
    /// rule a parser can try more than once at the same place needs.
    pub remembers: bool,
}

/// What a rule, or part of one, matches.
#[derive(Debug)]
pub enum Expression {
    /// Matches each expression one after another.
    Sequence(&'static [Expression]),
    /// Matches the branch that matches the most.
    Choice(&'static [Expression]),
    /// Matches the expression if it can.
    Optional(&'static Expression),
    /// Matches the expression as many times as it can.
    Repeated(&'static Expression),
    /// Matches the expression exactly this many times.
    Times(u32, &'static Expression),
    /// Matches what the first matches, unless the second matches all of it.
    Except(&'static Expression, &'static Expression),
    /// Matches the expression, which can only start with a character in one of
    /// the ranges, given in order. When the next character is in none of them,
    /// the expression is not tried at all, unless the parser is finding out
    /// why an input does not match.
    StartsWith(&'static [(char, char)], &'static Expression),
    /// Matches what the rule with this number matches.
    Rule(usize),
    Text(&'static str),
    /// Matches one character from the first to the second.
    Range(char, char),
    Empty,
}

/// Returns the tree of `input` matched in full by the rule numbered `rule`, or
/// where and why it does not match.
pub fn parse(
    grammar: &'static Grammar,
    rule: usize,
    input: &str,
) -> Result<Tree, Error> {
    let mut parser = Parser::new(grammar, input, false);
    match parser.rule(rule, 0) {
        Some((end, node)) if end == input.len() => Ok(Tree {
            input: Arc::new(Input::new(input)),
            nodes: parser.nodes,
            children: parser.children,
            root: node.expect("a start rule is never hidden"),
        }),
        _ => Err(explain(grammar, rule, input)),
    }
}

/// Matches `input` against the rule numbered `rule` again, which it does not
/// match in full, and returns where and why it does not.
fn explain(grammar: &'static Grammar, rule: usize, input: &str) -> Error {
    let mut parser = Parser::new(grammar, input, true);
    if let Some((end, _)) = parser.rule(rule, 0) {
        parser.expect(end, Expected::End);
    }
    parser.error()
}

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
    pub fn root(&self) -> Node<'_> {
        Node {
            tree: self,
            index: self.root,
        }
    }
}

#[derive(Debug)]
struct NodeData {
    rule: usize,
    start: usize,
    end: usize,
    /// Where the node's children are in the children of its tree.
    children: Range<usize>,
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
    pub fn rule(&self) -> usize {
        self.data().rule
    }

    pub fn span(&self) -> Span {
        let data = self.data();
        Span::new(Arc::clone(&self.tree.input), data.start..data.end)
    }

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
    pub fn child(&self) -> Option<Node<'t>> {
        self.children().next()
    }

    /// Returns the rule numbered `rule` matched inside this one, which the
    /// grammar makes sure is there.
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

    /// Returns every rule numbered `rule` matched inside this one, in order.
    pub fn many<T: FromNode>(&self, rule: usize) -> Vec<T> {
        self.children()
            .filter(|child| child.rule() == rule)
            .map(T::from_node)
            .collect()
    }
}

/// Builds the type of a rule from the node of its match.
pub trait FromNode {
    fn from_node(node: Node<'_>) -> Self;
}

impl<T: FromNode> FromNode for Box<T> {
    fn from_node(node: Node<'_>) -> Self {
        Box::new(T::from_node(node))
    }
}

/// Where a rule's match ends, with its node unless the rule is hidden, or
/// `None` when it does not match.
type Match = Option<(usize, Option<usize>)>;

/// Matches expressions against an input.
struct Parser<'i> {
    grammar: &'static Grammar,
    input: &'i str,
    nodes: Vec<NodeData>,
    /// The children of every node, those of each node next to each other.
    children: Vec<usize>,
    /// The nodes matched inside the rules being matched, those of each rule
    /// after those of the rules around it.
    matched: Vec<usize>,
    /// The match of each rule at each position. The keys are small numbers an
    /// input cannot pick freely, so a fast hasher that does not resist
    /// collisions suits them.
    matches: FxHashMap<(usize, usize), Match>,
    /// The furthest position where something failed to match.
    furthest: usize,
    /// What could have been read at the furthest position.
    expected: Vec<Expected>,
    /// Whether failures count towards the error, which they only do when
    /// finding out why an input does not match, and never inside an exception.
    reports: bool,
}

impl<'i> Parser<'i> {
    fn new(grammar: &'static Grammar, input: &'i str, reports: bool) -> Self {
        Self {
            grammar,
            input,
            nodes: Vec::new(),
            children: Vec::new(),
            matched: Vec::new(),
            matches: FxHashMap::default(),
            furthest: 0,
            expected: Vec::new(),
            reports,
        }
    }

    fn error(self) -> Error {
        Error::new(self.input, self.furthest, self.expected)
    }

    /// Notes that `expected` could have been read at `position`.
    fn expect(&mut self, position: usize, expected: Expected) {
        if !self.reports || position < self.furthest {
            return;
        }
        if position > self.furthest {
            self.furthest = position;
            self.expected.clear();
        }
        self.expected.push(expected);
    }

    /// Returns where the rule numbered `rule` matched at `position` ends, and
    /// its node unless the rule is hidden.
    fn rule(&mut self, rule: usize, position: usize) -> Match {
        let definition = &self.grammar.rules[rule];
        if definition.remembers {
            if let Some(&found) = self.matches.get(&(rule, position)) {
                return found;
            }
        }
        let earlier = if self.furthest == position {
            self.expected.len()
        } else {
            0
        };
        let mark = self.matched.len();
        let found = self
            .expression(&definition.expression, position)
            .map(|end| (end, self.node(rule, position..end, mark)));
        // What the rule could not read where it starts is told as the rule
        // itself, unless it is hidden.
        if self.reports
            && self.furthest == position
            && self.expected.len() > earlier
        {
            self.expected.truncate(earlier);
            if !definition.hidden {
                self.expected.push(Expected::Rule(definition.name));
            }
        }
        if definition.remembers {
            self.matches.insert((rule, position), found);
        }
        found
    }

    /// Returns the node of the rule numbered `rule` that matched `range`,
    /// whose children are the nodes matched since `mark`, or drops those nodes
    /// when the rule is hidden.
    fn node(
        &mut self,
        rule: usize,
        range: Range<usize>,
        mark: usize,
    ) -> Option<usize> {
        if self.grammar.rules[rule].hidden {
            self.matched.truncate(mark);
            return None;
        }
        let first = self.children.len();
        self.children.extend(self.matched.drain(mark..));
        self.nodes.push(NodeData {
            rule,
            start: range.start,
            end: range.end,
            children: first..self.children.len(),
        });
        Some(self.nodes.len() - 1)
    }

    /// Returns where `expression` matched at `position` ends, adding the nodes
    /// of the rules it matched to those matched so far. Adds nothing when it
    /// does not match.
    fn expression(
        &mut self,
        expression: &'static Expression,
        position: usize,
    ) -> Option<usize> {
        let mark = self.matched.len();
        let end = self.match_expression(expression, position);
        if end.is_none() {
            self.matched.truncate(mark);
        }
        end
    }

    fn match_expression(
        &mut self,
        expression: &'static Expression,
        position: usize,
    ) -> Option<usize> {
        match *expression {
            Expression::Text(text) => {
                if self.input[position..].starts_with(text) {
                    return Some(position + text.len());
                }
                self.expect(position, Expected::Text(text));
                None
            }
            Expression::Range(first, last) => {
                let next = self.next_character(position);
                if let Some(c) = next.filter(|c| (first..=last).contains(c)) {
                    return Some(position + c.len_utf8());
                }
                self.expect(position, Expected::Range(first, last));
                None
            }
            Expression::Empty => Some(position),
            Expression::Rule(rule) => {
                let (end, node) = self.rule(rule, position)?;
                self.matched.extend(node);
                Some(end)
            }
            Expression::Sequence(expressions) => {
                expressions.iter().try_fold(position, |end, expression| {
                    self.expression(expression, end)
                })
            }
            Expression::Times(times, expression) => (0..times)
                .try_fold(position, |end, _| self.expression(expression, end)),
            Expression::Choice(expressions) => {
                let mark = self.matched.len();
                let mut longest = None;
                for expression in expressions {
                    let start = self.matched.len();
                    let Some(end) = self.expression(expression, position)
                    else {
                        continue;
                    };
                    // Only the nodes of the longest branch so far stay.
                    if longest.is_none_or(|most| end > most) {
                        self.matched.drain(mark..start);
                        longest = Some(end);
                    } else {
                        self.matched.truncate(start);
                    }
                }
                longest
            }
            Expression::Optional(expression) => {
                Some(self.expression(expression, position).unwrap_or(position))
            }
            Expression::Repeated(expression) => {
                let mut end = position;
                loop {
                    let mark = self.matched.len();
                    match self.expression(expression, end) {
                        Some(next) if next > end => end = next,
                        _ => {
                            self.matched.truncate(mark);
                            return Some(end);
                        }
                    }
                }
            }
            Expression::StartsWith(ranges, expression) => {
                let starts = self.next_character(position).is_some_and(|c| {
                    let index = ranges.partition_point(|&(_, last)| last < c);
                    ranges.get(index).is_some_and(|&(first, _)| first <= c)
                });
                if !starts && !self.reports {
                    return None;
                }
                self.match_expression(expression, position)
            }
            Expression::Except(factor, exception) => {
                let end = self.expression(factor, position)?;
                // The exception only reads what the factor matched.
                let mut within =
                    Parser::new(self.grammar, &self.input[..end], false);
                let excluded =
                    within.expression(exception, position) == Some(end);
                (!excluded).then_some(end)
            }
        }
    }

    fn next_character(&self, position: usize) -> Option<char> {
        self.input[position..].chars().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `list = item, {',', item}; item = digit, {digit} | '(', list, ')';`
    /// with `digit` hidden.
    static LIST: Grammar = Grammar {
        rules: &[
            Rule {
                name: "list",
                expression: Expression::Sequence(&[
                    Expression::Rule(1),
                    Expression::Repeated(&Expression::StartsWith(
                        &[(',', ',')],
                        &Expression::Sequence(&[
                            Expression::Text(","),
                            Expression::Rule(1),
                        ]),
                    )),
                ]),
                hidden: false,
                remembers: false,
            },
            Rule {
                name: "item",
                expression: Expression::Choice(&[
                    Expression::StartsWith(
                        &[('0', '9')],
                        &Expression::Sequence(&[
                            Expression::Rule(2),
                            Expression::Repeated(&Expression::StartsWith(
                                &[('0', '9')],
                                &Expression::Rule(2),
                            )),
                        ]),
                    ),
                    Expression::StartsWith(
                        &[('(', '(')],
                        &Expression::Sequence(&[
                            Expression::Text("("),
                            Expression::Rule(0),
                            Expression::Text(")"),
                        ]),
                    ),
                ]),
                hidden: false,
                remembers: false,
            },
            Rule {
                name: "digit",
                expression: Expression::Range('0', '9'),
                hidden: true,
                remembers: false,
            },
        ],
    };

    /// Writes the tree under `node` as `rule(child, ...)`, with each leaf
    /// written as its text.
    fn outline(node: Node<'_>) -> String {
        let children: Vec<String> = node.children().map(outline).collect();
        if children.is_empty() {
            format!("{}:{}", node.rule(), node.text())
        } else {
            format!("{}({})", node.rule(), children.join(", "))
        }
    }

    #[test]
    fn builds_the_tree_of_an_input() {
        let tree = parse(&LIST, 0, "12,(3,4)").unwrap();
        assert_eq!(outline(tree.root()), "0(1:12, 1(0(1:3, 1:4)))");
        assert_eq!(tree.root().span().as_str(), "12,(3,4)");
    }

    #[test]
    fn reports_a_rule_that_could_not_start_by_its_name() {
        let error = parse(&LIST, 0, "1,(2,)").unwrap_err();
        assert_eq!(
            error.to_string(),
            "expected an item, found `)` at line 1, column 6"
        );
    }

    #[test]
    fn reports_an_input_that_goes_on_past_the_rule_without_hidden_rules() {
        let error = parse(&LIST, 0, "1)").unwrap_err();
        assert_eq!(
            error.to_string(),
            "expected `,` or the end of the input, found `)` at line 1, \
             column 2"
        );
    }

    /// `s = 'b' | 'c';`, where the table wrongly says the first branch can
    /// only start with `a`.
    static WRONG: Grammar = Grammar {
        rules: &[Rule {
            name: "s",
            expression: Expression::Choice(&[
                Expression::StartsWith(&[('a', 'a')], &Expression::Text("b")),
                Expression::Text("c"),
            ]),
            hidden: false,
            remembers: false,
        }],
    };

    #[test]
    fn skips_a_branch_that_cannot_start_with_the_next_character() {
        assert!(parse(&WRONG, 0, "c").is_ok());
        assert!(parse(&WRONG, 0, "b").is_err());
    }

    /// `s = a, 'x' | a, 'y'; a = 'z';`, where both branches reach `a` at the
    /// start, so it remembers its match.
    static SHARED: Grammar = Grammar {
        rules: &[
            Rule {
                name: "s",
                expression: Expression::Choice(&[
                    Expression::Sequence(&[
                        Expression::Rule(1),
                        Expression::Text("x"),
                    ]),
                    Expression::Sequence(&[
                        Expression::Rule(1),
                        Expression::Text("y"),
                    ]),
                ]),
                hidden: false,
                remembers: false,
            },
            Rule {
                name: "a",
                expression: Expression::Text("z"),
                hidden: false,
                remembers: true,
            },
        ],
    };

    #[test]
    fn matches_a_rule_that_remembers_once_at_each_place() {
        let tree = parse(&SHARED, 0, "zy").unwrap();
        assert_eq!(outline(tree.root()), "0(1:z)");
        assert_eq!(tree.nodes.len(), 2);
    }

    /// `word = keyword | name; keyword = 'if'; name = letter, {letter};`
    static WORDS: Grammar = Grammar {
        rules: &[
            Rule {
                name: "word",
                expression: Expression::Choice(&[
                    Expression::Rule(1),
                    Expression::Rule(2),
                ]),
                hidden: false,
                remembers: false,
            },
            Rule {
                name: "keyword",
                expression: Expression::Text("if"),
                hidden: false,
                remembers: false,
            },
            Rule {
                name: "name",
                expression: Expression::Sequence(&[
                    Expression::Range('a', 'z'),
                    Expression::Repeated(&Expression::Range('a', 'z')),
                ]),
                hidden: false,
                remembers: false,
            },
        ],
    };

    #[test]
    fn takes_the_branch_that_matches_the_most() {
        let tree = parse(&WORDS, 0, "iffy").unwrap();
        assert_eq!(outline(tree.root()), "0(2:iffy)");
    }

    #[test]
    fn takes_the_first_written_of_branches_that_tie() {
        let tree = parse(&WORDS, 0, "if").unwrap();
        assert_eq!(outline(tree.root()), "0(1:if)");
    }

    /// `word = letter - 'if', ['!'] | 2 * '?';`
    static EXCEPT: Grammar = Grammar {
        rules: &[Rule {
            name: "word",
            expression: Expression::Choice(&[
                Expression::Sequence(&[
                    Expression::Except(
                        &Expression::Sequence(&[
                            Expression::Range('a', 'z'),
                            Expression::Repeated(&Expression::Range('a', 'z')),
                        ]),
                        &Expression::Text("if"),
                    ),
                    Expression::Optional(&Expression::Text("!")),
                ]),
                Expression::Times(2, &Expression::Text("?")),
            ]),
            hidden: false,
            remembers: false,
        }],
    };

    #[test]
    fn leaves_out_what_an_exception_matches() {
        assert!(parse(&EXCEPT, 0, "in!").is_ok());
        assert!(parse(&EXCEPT, 0, "if").is_err());
        assert!(parse(&EXCEPT, 0, "iff").is_ok());
    }

    #[test]
    fn matches_a_factor_its_number_of_times() {
        assert!(parse(&EXCEPT, 0, "??").is_ok());
        assert!(parse(&EXCEPT, 0, "?").is_err());
    }

    /// `s = {['x']};`
    static NULLABLE: Grammar = Grammar {
        rules: &[Rule {
            name: "s",
            expression: Expression::Repeated(&Expression::Optional(
                &Expression::Text("x"),
            )),
            hidden: false,
            remembers: false,
        }],
    };

    #[test]
    fn stops_repeating_once_a_round_reads_nothing() {
        assert!(parse(&NULLABLE, 0, "").is_ok());
        assert!(parse(&NULLABLE, 0, "xx").is_ok());
    }

    #[test]
    fn finds_the_only_rule_matched_inside_a_node() {
        let tree = parse(&WORDS, 0, "if").unwrap();
        let child = tree.root().child().unwrap();
        assert_eq!((child.rule(), child.text()), (1, "if"));
        assert!(child.child().is_none());
    }

    /// The type of a rule of [`LIST`].
    #[derive(Debug, PartialEq)]
    struct Item {
        list: Option<Box<List>>,
        text: String,
    }

    #[derive(Debug, PartialEq)]
    struct List {
        items: Vec<Item>,
    }

    impl FromNode for Item {
        fn from_node(node: Node<'_>) -> Self {
            Self {
                list: node.optional(0),
                text: node.text().to_owned(),
            }
        }
    }

    impl FromNode for List {
        fn from_node(node: Node<'_>) -> Self {
            Self {
                items: node.many(1),
            }
        }
    }

    #[test]
    fn builds_types_from_the_nodes() {
        let tree = parse(&LIST, 0, "1,(2)").unwrap();
        let list = List::from_node(tree.root());
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].text, "1");
        let inner = list.items[1].list.as_ref().unwrap();
        assert_eq!(inner.items[0].text, "2");
    }
}
