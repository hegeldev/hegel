//! Shrinking a string under a grammar through its derivation. The string
//! passes leave such a string alone — the strings they would propose are
//! hardly ever in the grammar — and this pass edits it where its
//! derivation says the grammar might still accept the result: a rule's
//! text replaced by that of a smaller node of the same rule inside it,
//! repetitions deleted, ignored text cut, a rule's or a terminal's text
//! replaced by the smallest the constraint has seen it derive, and each
//! leaf's text shortened and lowered character by character. Every
//! candidate is a splice of the text that the parser then judges, so an
//! edit that leaves the grammar (a lexer reading two neighbouring tokens
//! as one, say) is simply rejected.

use super::search::BinSearchDown;
use super::{PassExit, ShrinkResult, Shrinker, absorb_node_gone};
use crate::control::hegel_internal_unwrap;
use crate::native::HashMap;
use crate::native::core::{ChoiceValue, StringChoice};
use crate::native::lark::constraint::GrammarConstraint;
use crate::native::lark::layout::{Layout, cut, splice};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// A string under a grammar as the pass sees it between probes: its
/// kind, its constraint, its text and the derivation laid over the text.
struct Derived {
    kind: StringChoice,
    constraint: Arc<GrammarConstraint>,
    text: Vec<u32>,
    layout: Layout,
}

impl<'a> Shrinker<'a> {
    pub(super) async fn shrink_grammar_strings(&mut self) -> ShrinkResult<()> {
        let mut i = 0;
        while i < self.current_nodes.len() {
            absorb_node_gone(self.shrink_grammar_node(i).await)?;
            i += 1;
        }
        Ok(())
    }

    async fn shrink_grammar_node(&mut self, i: usize) -> Result<(), PassExit> {
        let (kind, _, current) = self.grammar_string_at(i).ok_or(PassExit::NodeGone)?;
        let simplest = kind.simplest()?;
        if simplest != current {
            self.replace_text(i, simplest).await?;
        }
        loop {
            let mut progress = self.hoist_rules(i).await?;
            progress |= self.delete_repetitions(i).await?;
            progress |= self.delete_ignored(i).await?;
            progress |= self.replace_rules_with_smallest(i).await?;
            progress |= self.shrink_leaves(i).await?;
            if !progress {
                return Ok(());
            }
        }
    }

    /// The kind, constraint and value at node `i`, or `None` when the
    /// node is not (or no longer) a string under a grammar.
    fn grammar_string_at(
        &self,
        i: usize,
    ) -> Option<(StringChoice, Arc<GrammarConstraint>, Vec<u32>)> {
        let (sc, v) = self.current_nodes.get(i)?.data.as_string()?;
        let constraint = Arc::clone(sc.grammar.as_ref()?);
        Some((sc.clone(), constraint, v.to_vec()))
    }

    /// Node `i` with its derivation; `NodeGone` when it is no string under
    /// a grammar, or holds a string the grammar does not derive.
    fn derivation(&self, i: usize) -> Result<Derived, PassExit> {
        let (kind, constraint, text) = self.grammar_string_at(i).ok_or(PassExit::NodeGone)?;
        let tree = constraint.tree(&text).ok_or(PassExit::NodeGone)?;
        let layout = Layout::new(&tree);
        Ok(Derived {
            kind,
            constraint,
            text,
            layout,
        })
    }

    /// Try `text` at node `i`; a text equal to the current one is no
    /// shrink and is not run.
    async fn replace_text(&mut self, i: usize, text: Vec<u32>) -> Result<bool, PassExit> {
        let (_, _, current) = self.grammar_string_at(i).ok_or(PassExit::NodeGone)?;
        if current == text {
            return Ok(false);
        }
        Ok(self
            .replace(&HashMap::from_iter([(i, ChoiceValue::String(text))]))
            .await?)
    }

    /// Replace a rule node's text with that of a node of the same rule
    /// inside it, outermost first, starting over after each accepted
    /// hoist.
    async fn hoist_rules(&mut self, i: usize) -> Result<bool, PassExit> {
        let mut changed = false;
        'again: loop {
            let d = self.derivation(i)?;
            for (ancestor, descendant) in d.layout.hoists() {
                let (a, b) = (&d.layout.nodes()[ancestor], &d.layout.nodes()[descendant]);
                let candidate = splice(&d.text, a.start, a.end, &d.text[b.start..b.end]);
                if self.replace_text(i, candidate).await? {
                    changed = true;
                    continue 'again;
                }
            }
            return Ok(changed);
        }
    }

    /// For each repeat, delete all its repetitions, then windows of 8, 4,
    /// 2 and 1 of them from the back.
    async fn delete_repetitions(&mut self, i: usize) -> Result<bool, PassExit> {
        let mut changed = false;
        let mut r = 0;
        loop {
            let d = self.derivation(i)?;
            let Some(items) = d.layout.repeats().get(r).cloned() else {
                return Ok(changed);
            };
            if let (Some(first), Some(last)) = (items.first(), items.last()) {
                if self
                    .replace_text(i, splice(&d.text, first.0, last.1, &[]))
                    .await?
                {
                    changed = true;
                }
            }
            for k in [8usize, 4, 2, 1] {
                let mut j = usize::MAX;
                loop {
                    let d = self.derivation(i)?;
                    let Some(items) = d.layout.repeats().get(r).cloned() else {
                        return Ok(changed);
                    };
                    j = j.min(items.len());
                    if j < k || k >= items.len() {
                        break;
                    }
                    let candidate = splice(&d.text, items[j - k].0, items[j - 1].1, &[]);
                    j -= if self.replace_text(i, candidate).await? {
                        changed = true;
                        k
                    } else {
                        1
                    };
                }
            }
            r += 1;
        }
    }

    /// Cut all the ignored text at once, then each stretch of it from the
    /// back.
    async fn delete_ignored(&mut self, i: usize) -> Result<bool, PassExit> {
        let d = self.derivation(i)?;
        let ignored = d.layout.ignored();
        if ignored.is_empty() {
            return Ok(false);
        }
        let mut changed = self.replace_text(i, cut(&d.text, &ignored)).await?;
        let mut j = usize::MAX;
        loop {
            let d = self.derivation(i)?;
            let ignored = d.layout.ignored();
            j = j.min(ignored.len());
            if j == 0 {
                return Ok(changed);
            }
            j -= 1;
            let (start, end) = ignored[j];
            changed |= self
                .replace_text(i, splice(&d.text, start, end, &[]))
                .await?;
        }
    }

    /// Replace each rule node's text, outermost first, with the smallest
    /// text the constraint has seen the rule derive.
    async fn replace_rules_with_smallest(&mut self, i: usize) -> Result<bool, PassExit> {
        let mut changed = false;
        let mut r = 0;
        loop {
            let d = self.derivation(i)?;
            let Some(&(rule, start, end)) = d.layout.rules().get(r) else {
                return Ok(changed);
            };
            r += 1;
            if let Some(small) = d.constraint.smallest_for_rule(rule) {
                if d.constraint.smaller(&small, &d.text[start..end]) {
                    changed |= self
                        .replace_text(i, splice(&d.text, start, end, &small))
                        .await?;
                }
            }
        }
    }

    /// Replace each leaf's text with the smallest the constraint has seen
    /// its terminal match, and shrink the text of those it cannot.
    async fn shrink_leaves(&mut self, i: usize) -> Result<bool, PassExit> {
        let mut changed = false;
        let mut j = 0;
        loop {
            let d = self.derivation(i)?;
            let Some(&(terminal, start, end)) = d.layout.leaves().get(j) else {
                return Ok(changed);
            };
            j += 1;
            if let Some(small) = d.constraint.smallest_for_terminal(terminal) {
                if d.constraint.smaller(&small, &d.text[start..end])
                    && self
                        .replace_text(i, splice(&d.text, start, end, &small))
                        .await?
                {
                    changed = true;
                    continue;
                }
            }
            changed |= self.shrink_leaf(i, j - 1).await?;
        }
    }

    /// Node `i` with the range of its leaf `j`, or `None` when the
    /// derivation has fewer leaves now.
    fn leaf(&self, i: usize, j: usize) -> Result<Option<(Derived, usize, usize)>, PassExit> {
        let d = self.derivation(i)?;
        let range = d
            .layout
            .leaves()
            .get(j)
            .map(|&(_, start, end)| (start, end));
        Ok(range.map(|(start, end)| (d, start, end)))
    }

    /// Shrink the text of leaf `j`: delete it, keep the shortest prefix
    /// that will do, delete single characters from the back, then lower
    /// each character as far down the alphabet's shrink order as it goes.
    async fn shrink_leaf(&mut self, i: usize, j: usize) -> Result<bool, PassExit> {
        let Some((d, start, end)) = self.leaf(i, j)? else {
            return Ok(false);
        };
        if start == end {
            return Ok(false);
        }
        if self
            .replace_text(i, splice(&d.text, start, end, &[]))
            .await?
        {
            return Ok(true);
        }
        let mut changed = false;
        let mut search = BinSearchDown::new(1, (end - start) as i128);
        while let Some(len) = search.probe() {
            let Some((d, start, end)) = self.leaf(i, j)? else {
                return Ok(changed);
            };
            let len = len as usize;
            let ok = len < end - start
                && self
                    .replace_text(i, splice(&d.text, start + len, end, &[]))
                    .await?;
            search.record(ok);
            changed |= ok;
        }
        let mut p = usize::MAX;
        loop {
            let Some((d, start, end)) = self.leaf(i, j)? else {
                return Ok(changed);
            };
            p = p.min(end - start);
            if p <= 1 {
                break;
            }
            p -= 1;
            changed |= self
                .replace_text(i, splice(&d.text, start + p, start + p + 1, &[]))
                .await?;
        }
        let mut p = 0;
        loop {
            let Some((d, start, end)) = self.leaf(i, j)? else {
                return Ok(changed);
            };
            if p >= end - start {
                return Ok(changed);
            }
            let key = d.kind.codepoint_key(d.text[start + p]);
            let mut search = BinSearchDown::new(0, key as i128);
            while let Some(k) = search.probe().filter(|_| key > 0) {
                let Some((d, start, end)) = self.leaf(i, j)? else {
                    return Ok(changed);
                };
                let codepoint = hegel_internal_unwrap!(
                    d.kind.key_to_codepoint(k as u32),
                    "shrink_leaf: a key below a character's is in the alphabet"
                );
                let ok = p < end - start
                    && self
                        .replace_text(i, splice(&d.text, start + p, start + p + 1, &[codepoint]))
                        .await?;
                search.record(ok);
                changed |= ok;
            }
            p += 1;
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/shrinker_grammar_tests.rs"]
mod tests;
