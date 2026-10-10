//! The grammar constraint a string choice can carry: a string drawn from
//! a Lark grammar is one string node in the choice sequence, and its
//! kind, a [`StringChoice`](crate::native::core::StringChoice) with a
//! [`GrammarConstraint`], accepts exactly the strings the grammar derives.
//! Validating a replayed string means parsing it, so the constraint owns
//! the compiled grammar, its terminals as the parser matches them (see
//! [`GrammarTerminals`]) and a [`Parser`] from its start rule, and
//! remembers the derivations it has found so that a string known to
//! derive is never parsed twice. From every derivation it also learns
//! the smallest text it has seen each rule and each terminal derive,
//! which grammar-directed shrinking proposes in place of a larger one.
//!
//! Two constraints compare equal when they were compiled from the same
//! grammar, for the same start rule and size cap: the engine compares
//! kinds across executions (see
//! [`KindLedger`](crate::native::exec_cache::KindLedger)), and a test
//! that compiles its grammar afresh each run must not look nondeterministic.

use super::compile::{Grammar, RuleId, TermId};
use super::explicit::GrammarTerminals;
use super::layout::{Layout, Part};
use super::parse::{Parser, Tree};
use crate::control::InternalError;
use crate::native::HashMap;
use crate::native::draws::StringSpec;
use crate::native::intervalsets::IntervalSet;
use crate::sys::sync::Mutex;
use alloc::boxed::Box;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};
use once_cell::race::OnceBox;
use rustc_hash::FxHasher;

/// How many derivations a constraint remembers before forgetting them all
/// to make room.
pub const KNOWN_CAPACITY: usize = 512;

/// A grammar constraint on a string choice; see the module docs.
pub struct GrammarConstraint {
    grammar: Grammar,
    terminals: GrammarTerminals,
    parser: Parser,
    start: RuleId,
    identity: u64,
    max_size: usize,
    /// The simplest string, computed by the generator at the first draw
    /// (see [`GrammarConstraint::simplest_or_init`]).
    simplest: OnceBox<Vec<u32>>,
    known: Mutex<HashMap<(u64, usize), Arc<Tree>>>,
    intervals: Arc<IntervalSet>,
    smallest: Mutex<Smallest>,
}

/// The smallest text seen derived by each rule and each terminal, in
/// the shrink order of the strings the constraint is on: shorter first,
/// then character by character in the alphabet's shrink order.
#[derive(Default)]
struct Smallest {
    rules: Vec<Option<Vec<u32>>>,
    terminals: Vec<Option<Vec<u32>>>,
}

impl GrammarConstraint {
    /// The constraint accepting the strings `grammar` derives from the
    /// rule `start` within `max_size` characters, with the terminals in
    /// `explicit` matched by the generators given for them. Its simplest
    /// string is the generator's to supply (see
    /// [`simplest_or_init`](Self::simplest_or_init)) before any choice
    /// is drawn under it.
    pub fn new(
        grammar: Grammar,
        explicit: &[(TermId, Arc<StringSpec>)],
        start: RuleId,
        max_size: usize,
    ) -> Result<GrammarConstraint, InternalError> {
        let terminals = GrammarTerminals::new(&grammar, explicit)?;
        let parser = Parser::new(&grammar, start);
        let intervals = IntervalSet::new(alloc::vec![(0, 0xD7FF), (0xE000, 0x10FFFF)])?;
        let mut hasher = FxHasher::default();
        format!("{grammar:?}").hash(&mut hasher);
        start.hash(&mut hasher);
        max_size.hash(&mut hasher);
        let smallest = Smallest {
            rules: alloc::vec![None; grammar.rules.len()],
            terminals: alloc::vec![None; grammar.terminals.len()],
        };
        Ok(GrammarConstraint {
            grammar,
            terminals,
            parser,
            start,
            identity: hasher.finish(),
            max_size,
            simplest: OnceBox::new(),
            known: Mutex::new(HashMap::default()),
            intervals: Arc::new(intervals),
            smallest: Mutex::new(smallest),
        })
    }

    /// The alphabet of the strings the constraint is on: every Unicode
    /// scalar value, the grammar alone deciding which strings are in.
    pub fn intervals(&self) -> &Arc<IntervalSet> {
        &self.intervals
    }

    /// The simplest string the grammar derives, computing it with `init`
    /// when it is not set yet: the one the generator draws when every
    /// decision goes the simplest way, which stands in for a replayed
    /// string the grammar rejects. Once set it is never computed again.
    pub fn simplest_or_init<E>(
        &self,
        init: impl FnOnce() -> Result<Vec<u32>, E>,
    ) -> Result<&[u32], E> {
        let simplest = self
            .simplest
            .get_or_try_init(|| init().map(Box::new))
            .map(|simplest| simplest.as_slice())?;
        self.tree(simplest);
        Ok(simplest)
    }

    /// Whether `text` is in the grammar: known to derive, or parsed now
    /// and remembered. A codepoint that is not a Unicode scalar value is
    /// in no grammar.
    pub fn accepts(&self, text: &[u32]) -> bool {
        self.tree(text).is_some()
    }

    /// A derivation of `text`, remembered or found now, or `None` when
    /// the grammar does not derive it.
    pub fn tree(&self, text: &[u32]) -> Option<Arc<Tree>> {
        let key = text_key(text);
        if let Some(tree) = self.known.lock().get(&key) {
            return Some(Arc::clone(tree));
        }
        let chars: Vec<char> = text
            .iter()
            .map(|&cp| char::from_u32(cp))
            .collect::<Option<_>>()?;
        let tree = Arc::new(self.parser.parse(&self.terminals, &chars).ok()?);
        self.learn(&tree, text);
        let mut known = self.known.lock();
        if known.len() >= KNOWN_CAPACITY {
            known.clear();
        }
        known.insert(key, Arc::clone(&tree));
        Some(tree)
    }

    /// Note the text each rule and terminal derives in `tree`, a
    /// derivation of `text`, where it is smaller than any seen before.
    fn learn(&self, tree: &Tree, text: &[u32]) {
        let mut smallest = self.smallest.lock();
        for node in Layout::new(tree).nodes() {
            let slot = match node.part {
                Part::Rule(id) => &mut smallest.rules[id],
                Part::Terminal(id) | Part::Ignored(id) => &mut smallest.terminals[id],
                Part::Group | Part::Repeat => continue,
            };
            let derived = &text[node.start..node.end];
            if slot
                .as_deref()
                .is_none_or(|known| self.smaller(derived, known))
            {
                *slot = Some(derived.to_vec());
            }
        }
    }

    /// Whether `a` comes before `b` in the shrink order of the strings
    /// the constraint is on.
    pub fn smaller(&self, a: &[u32], b: &[u32]) -> bool {
        let key = |cp: &u32| self.intervals.index_from_codepoint_in_shrink_order(*cp);
        a.len() < b.len() || (a.len() == b.len() && a.iter().map(key).lt(b.iter().map(key)))
    }

    /// The smallest text seen derived by `rule`.
    pub fn smallest_for_rule(&self, rule: RuleId) -> Option<Vec<u32>> {
        self.smallest.lock().rules[rule].clone()
    }

    /// The smallest text seen matched by `terminal`.
    pub fn smallest_for_terminal(&self, terminal: TermId) -> Option<Vec<u32>> {
        self.smallest.lock().terminals[terminal].clone()
    }

    /// The simplest string the grammar derives, once set.
    pub fn simplest(&self) -> Option<&[u32]> {
        self.simplest.get().map(|simplest| simplest.as_slice())
    }

    /// The hash of the compiled grammar, start rule and size cap that
    /// the constraint compares and hashes by.
    pub fn identity(&self) -> u64 {
        self.identity
    }

    /// The cap on the length of the strings accepted.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// The compiled grammar.
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    /// The start rule.
    pub fn start(&self) -> RuleId {
        self.start
    }

    /// How many derivations are remembered.
    #[cfg(test)]
    pub fn known_len(&self) -> usize {
        self.known.lock().len()
    }
}

/// The key `text` is remembered under: its hash, and its length so that
/// a string of another length never reads as it.
fn text_key(text: &[u32]) -> (u64, usize) {
    let mut hasher = FxHasher::default();
    for &cp in text {
        hasher.write_u32(cp);
    }
    (hasher.finish(), text.len())
}

impl PartialEq for GrammarConstraint {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Eq for GrammarConstraint {}

impl Hash for GrammarConstraint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

impl fmt::Debug for GrammarConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrammarConstraint")
            .field("identity", &format_args!("{:016x}", self.identity))
            .field("start", &self.grammar.rules[self.start].name)
            .finish()
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/constraint_tests.rs"]
mod tests;
