//! Where a derivation's parts lie in the text it derives: the view of a
//! [`Tree`] that grammar-directed shrinking edits through. Every subtree
//! gets the range of text its leaves cover, and the layout answers the
//! structural questions the shrinker asks — which rule nodes contain a
//! shorter node of the same rule (a hoist), what the repetitions of each
//! repeat are, where the ignored text lies and which leaves make up the
//! text — so that the shrinker's candidates are splices of the text and
//! the parser decides which of them are still in the grammar.

use super::compile::{RuleId, TermId};
use super::parse::Tree;
use crate::native::HashSet;
use alloc::vec::Vec;

/// What a node of the layout is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Rule(RuleId),
    Group,
    Repeat,
    Terminal(TermId),
    Ignored(TermId),
}

/// A subtree: what it is, the text it derives — from its first to its
/// last terminal, so ignored text at its edges belongs to no node but
/// its own leaf — and its children (as indices into the layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub part: Part,
    pub start: usize,
    pub end: usize,
    pub children: Vec<usize>,
}

/// The nodes of a derivation in pre-order, each with its range of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    nodes: Vec<Node>,
}

impl Layout {
    pub fn new(tree: &Tree) -> Layout {
        let mut layout = Layout { nodes: Vec::new() };
        let mut derives = Vec::new();
        layout.place(tree, 0, &mut derives);
        layout
    }

    /// Lay `tree` out from text position `at`, returning the position
    /// after it; `derives` records per node whether it derives any text
    /// of its own, which is what its range covers.
    fn place(&mut self, tree: &Tree, at: usize, derives: &mut Vec<bool>) -> usize {
        let index = self.nodes.len();
        let part = match tree {
            Tree::Terminal { id, .. } => Part::Terminal(*id),
            Tree::Ignored { id, .. } => Part::Ignored(*id),
            Tree::Rule { id, .. } => Part::Rule(*id),
            Tree::Group { .. } => Part::Group,
            Tree::Repeat { .. } => Part::Repeat,
        };
        self.nodes.push(Node {
            part,
            start: at,
            end: at,
            children: Vec::new(),
        });
        derives.push(false);
        match tree {
            Tree::Terminal { start, end, .. } | Tree::Ignored { start, end, .. } => {
                self.nodes[index].end = at + end - start;
                derives[index] = matches!(tree, Tree::Terminal { .. });
                self.nodes[index].end
            }
            Tree::Rule { .. } | Tree::Group { .. } | Tree::Repeat { .. } => {
                let mut cursor = at;
                for child in tree.children() {
                    let child_index = self.nodes.len();
                    cursor = self.place(child, cursor, derives);
                    self.nodes[index].children.push(child_index);
                    if derives[child_index] {
                        if !derives[index] {
                            self.nodes[index].start = self.nodes[child_index].start;
                            derives[index] = true;
                        }
                        self.nodes[index].end = self.nodes[child_index].end;
                    }
                }
                cursor
            }
        }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The pairs of a rule node and a node of the same rule inside it
    /// covering strictly less text, as `(ancestor, descendant)` indices:
    /// outermost ancestor first, and for each ancestor the descendant
    /// covering the least text first.
    pub fn hoists(&self) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        let mut ancestors: Vec<(usize, RuleId)> = Vec::new();
        self.collect_hoists(0, &mut ancestors, &mut pairs);
        pairs.sort_by_key(|&(a, d)| (a, self.nodes[d].end - self.nodes[d].start));
        pairs
    }

    fn collect_hoists(
        &self,
        index: usize,
        ancestors: &mut Vec<(usize, RuleId)>,
        pairs: &mut Vec<(usize, usize)>,
    ) {
        let node = &self.nodes[index];
        let rule = match node.part {
            Part::Rule(id) => Some(id),
            _ => None,
        };
        if let Some(id) = rule {
            for &(ancestor, ancestor_rule) in ancestors.iter() {
                let above = &self.nodes[ancestor];
                if ancestor_rule == id && node.end - node.start < above.end - above.start {
                    pairs.push((ancestor, index));
                }
            }
            ancestors.push((index, id));
        }
        for &child in &node.children {
            self.collect_hoists(child, ancestors, pairs);
        }
        if rule.is_some() {
            ancestors.pop();
        }
    }

    /// For each repeat node in pre-order, the ranges of its repetitions
    /// (its children other than the ignored text between them) in order.
    pub fn repeats(&self) -> Vec<Vec<(usize, usize)>> {
        self.nodes
            .iter()
            .filter(|node| node.part == Part::Repeat)
            .map(|node| {
                node.children
                    .iter()
                    .map(|&child| &self.nodes[child])
                    .filter(|child| !matches!(child.part, Part::Ignored(_)))
                    .map(|child| (child.start, child.end))
                    .collect()
            })
            .collect()
    }

    /// The ranges of ignored text, in text order.
    pub fn ignored(&self) -> Vec<(usize, usize)> {
        self.nodes
            .iter()
            .filter(|node| matches!(node.part, Part::Ignored(_)))
            .map(|node| (node.start, node.end))
            .collect()
    }

    /// Every leaf — a terminal's text or ignored text — with the terminal
    /// it matched, in text order.
    pub fn leaves(&self) -> Vec<(TermId, usize, usize)> {
        self.nodes
            .iter()
            .filter_map(|node| match node.part {
                Part::Terminal(id) | Part::Ignored(id) => Some((id, node.start, node.end)),
                _ => None,
            })
            .collect()
    }

    /// Every rule node with its rule, in pre-order.
    pub fn rules(&self) -> Vec<(RuleId, usize, usize)> {
        self.nodes
            .iter()
            .filter_map(|node| match node.part {
                Part::Rule(id) => Some((id, node.start, node.end)),
                _ => None,
            })
            .collect()
    }

    /// The coarse moves on `text` for a shrinker that tries each once
    /// rather than searching: all the ignored text cut, every repeat
    /// emptied, halved and shortened by one, every hoist, and every leaf
    /// replaced by the smallest text `smallest` knows for its terminal;
    /// each once, and none that leaves `text` as it is.
    pub fn coarse_candidates(
        &self,
        text: &[u32],
        smallest: impl Fn(TermId) -> Option<Vec<u32>>,
    ) -> Vec<Vec<u32>> {
        let mut candidates = Vec::new();
        let mut seen: HashSet<Vec<u32>> = HashSet::default();
        let mut propose = |candidate: Vec<u32>| {
            if candidate != text && seen.insert(candidate.clone()) {
                candidates.push(candidate);
            }
        };
        let ignored = self.ignored();
        if !ignored.is_empty() {
            propose(cut(text, &ignored));
        }
        for items in self.repeats() {
            let Some(last) = items.last() else {
                continue;
            };
            let half = items.len() / 2;
            propose(splice(text, items[0].0, last.1, &[]));
            if half > 0 {
                propose(splice(text, items[items.len() - half].0, last.1, &[]));
                propose(splice(text, items[items.len() - 1].0, last.1, &[]));
            }
        }
        for (ancestor, descendant) in self.hoists() {
            let (a, d) = (&self.nodes[ancestor], &self.nodes[descendant]);
            propose(splice(text, a.start, a.end, &text[d.start..d.end]));
        }
        for (id, start, end) in self.leaves() {
            if let Some(small) = smallest(id) {
                propose(splice(text, start, end, &small));
            }
        }
        candidates
    }
}

/// `text` with `start..end` replaced by `with`.
pub fn splice(text: &[u32], start: usize, end: usize, with: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(text.len() - (end - start) + with.len());
    out.extend_from_slice(&text[..start]);
    out.extend_from_slice(with);
    out.extend_from_slice(&text[end..]);
    out
}

/// `text` with each of `ranges` (disjoint, in text order) removed.
pub fn cut(text: &[u32], ranges: &[(usize, usize)]) -> Vec<u32> {
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    for &(start, end) in ranges {
        out.extend_from_slice(&text[at..start]);
        at = end;
    }
    out.extend_from_slice(&text[at..]);
    out
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/layout_tests.rs"]
mod tests;
