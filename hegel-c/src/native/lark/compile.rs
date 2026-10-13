//! Compiling a built grammar into the form the generator walks: Lark's
//! `Grammar.compile`, minus the EBNF-to-BNF desugaring. Terminals become a
//! single regex each (`TerminalTreeToPattern`), literals inside rules
//! become anonymous terminals (`PrepareAnonTerminals`), templates are
//! instantiated (`ApplyTemplates`), and rules unreachable from the start
//! symbols are dropped along with the terminals only they used.

use super::ast::{Expr, Pattern, PatternKind, Symbol};
use super::builder::BuiltGrammar;
use super::escape::pattern_to_regexp;
use super::{GrammarError, GrammarResult};
use crate::native::re::parser::{ParsedPattern, parse_pattern};
use crate::native::re::width::get_width;
use crate::native::{HashMap, HashSet};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub type RuleId = usize;
pub type TermId = usize;

/// How deeply a template instantiation may nest other instantiations in
/// its arguments (`t{t{t{x}}}`). A template whose body instantiates
/// itself around its own parameter (`t{x}: x | t{t{x}}`) would otherwise
/// be instantiated without end, one level deeper each time, as Lark does.
const MAX_TEMPLATE_NESTING: usize = 200;

/// A compiled terminal. `pattern` is `None` for a `%declare`d terminal,
/// whose strings must be supplied from outside the grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalDef {
    pub name: String,
    pub pattern: Option<Pattern>,
    /// The pattern as a Python-syntax regex (Lark's `to_regexp`), when
    /// there is one, and that regex parsed.
    pub regexp: Option<String>,
    pub parsed: Option<ParsedPattern>,
    pub priority: i64,
    /// The shortest and longest strings the terminal can match, in
    /// codepoints (`None` for unbounded), when there is a pattern.
    pub min_width: u64,
    pub max_width: Option<u64>,
}

/// The noise every placeholder marker carries, so that a grammar's own
/// text coincides with one only by design.
const PLACEHOLDER_NONCE: &str = "392f5a5e9326";

/// The marker standing for each value of the explicit terminal `name`
/// in a drawn string (see [`TerminalDef::placeholder`]).
pub fn placeholder(name: &str) -> String {
    format!("<<EXPLICIT_{name}_{PLACEHOLDER_NONCE}>>")
}

impl TerminalDef {
    /// The string terminal standing for the explicit terminal `name`:
    /// its [`placeholder`] marker, above every other terminal in
    /// priority so that the lexers read it as itself wherever it starts.
    pub fn placeholder(name: &str) -> GrammarResult<TerminalDef> {
        let pattern = Pattern {
            kind: PatternKind::Str,
            value: placeholder(name),
            flags: String::new(),
        };
        TerminalDef::new(name.to_string(), Some(pattern), i64::MAX)
    }

    /// Whether the terminal is an explicit terminal's placeholder.
    pub fn is_placeholder(&self) -> bool {
        self.pattern
            .as_ref()
            .is_some_and(|p| p.kind == PatternKind::Str && p.value == placeholder(&self.name))
    }

    /// The terminal `name` matching `pattern` (`None` for a `%declare`d
    /// one) with `priority`.
    pub fn new(
        name: String,
        pattern: Option<Pattern>,
        priority: i64,
    ) -> GrammarResult<TerminalDef> {
        let (regexp, parsed, min_width, max_width) = match &pattern {
            Some(p) => {
                let parsed = parse_terminal_pattern(&name, p)?;
                let (min, max) = get_width(&parsed.pattern);
                if min == 0 {
                    return Err(GrammarError::new(format!(
                        "Zero-width terminals are not allowed ({name}: {})",
                        pattern_to_regexp(p)
                    )));
                }
                (Some(pattern_to_regexp(p)), Some(parsed), min, max)
            }
            None => (None, None, 0, Some(0)),
        };
        Ok(TerminalDef {
            name,
            pattern,
            regexp,
            parsed,
            priority,
            min_width,
            max_width,
        })
    }
}

/// One element of a rule's right-hand side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Terminal(TermId),
    Rule(RuleId),
    /// A parenthesized group of alternatives.
    Group(Vec<Sequence>),
    /// Between `min` and `max` (unbounded when `None`) repetitions of
    /// `inner`.
    Repeat {
        inner: Box<Item>,
        min: u32,
        max: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sequence {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub alternatives: Vec<Sequence>,
}

/// A compiled grammar: rules and terminals by index, the terminals
/// `%ignore` lets appear anywhere, and the start rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grammar {
    pub rules: Vec<Rule>,
    pub terminals: Vec<TerminalDef>,
    pub ignore: Vec<TermId>,
    pub start: Vec<RuleId>,
}

impl Grammar {
    #[cfg(test)]
    pub fn rule_id(&self, name: &str) -> Option<RuleId> {
        self.rules.iter().position(|r| r.name == name)
    }

    pub fn terminal_id(&self, name: &str) -> Option<TermId> {
        self.terminals.iter().position(|t| t.name == name)
    }
}

/// Lark's `_TERMINAL_NAMES`: the names given to anonymous terminals for
/// common punctuation.
fn punctuation_name(value: &str) -> Option<&'static str> {
    Some(match value {
        "." => "DOT",
        "," => "COMMA",
        ":" => "COLON",
        ";" => "SEMICOLON",
        "+" => "PLUS",
        "-" => "MINUS",
        "*" => "STAR",
        "/" => "SLASH",
        "\\" => "BACKSLASH",
        "|" => "VBAR",
        "?" => "QMARK",
        "!" => "BANG",
        "@" => "AT",
        "#" => "HASH",
        "$" => "DOLLAR",
        "%" => "PERCENT",
        "^" => "CIRCUMFLEX",
        "&" => "AMPERSAND",
        "_" => "UNDERSCORE",
        "<" => "LESSTHAN",
        ">" => "MORETHAN",
        "=" => "EQUAL",
        "\"" => "DBLQUOTE",
        "'" => "QUOTE",
        "`" => "BACKQUOTE",
        "~" => "TILDE",
        "(" => "LPAR",
        ")" => "RPAR",
        "{" => "LBRACE",
        "}" => "RBRACE",
        "[" => "LSQB",
        "]" => "RSQB",
        "\n" => "NEWLINE",
        "\r\n" => "CRLF",
        "\t" => "TAB",
        " " => "SPACE",
        _ => return None,
    })
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// A pattern's regex form parsed with the engine's Python-syntax regex
/// parser.
fn parse_terminal_pattern(name: &str, pattern: &Pattern) -> GrammarResult<ParsedPattern> {
    parse_pattern(&pattern_to_regexp(pattern), 0)
        .map_err(|e| GrammarError::new(format!("Cannot compile token {name}: {e}")))
}

/// The widths of a pattern.
fn pattern_widths(name: &str, pattern: &Pattern) -> GrammarResult<(u64, Option<u64>)> {
    Ok(get_width(&parse_terminal_pattern(name, pattern)?.pattern))
}

/// The regex quantifier for a repetition. The parser builds an unbounded
/// one only from `*` or `+`.
fn repeat_suffix(min: u32, max: Option<u32>) -> String {
    match (min, max) {
        (0, Some(1)) => "?".to_string(),
        (0, None) => "*".to_string(),
        (_, None) => "+".to_string(),
        (n, Some(m)) if n == m => format!("{{{n}}}"),
        (n, Some(m)) => format!("{{{n},{m}}}"),
    }
}

/// Lark's `TerminalTreeToPattern`: fold a terminal's body into one
/// pattern. `name` is for diagnostics.
fn terminal_tree_to_pattern(name: &str, expr: &Expr) -> GrammarResult<Pattern> {
    match expr {
        Expr::Alternatives(alts) => {
            if alts.iter().any(|a| a.alias.is_some()) {
                return Err(GrammarError::new(
                    "Aliasing not allowed in terminals (You used -> in the wrong place)",
                ));
            }
            let mut patterns = Vec::with_capacity(alts.len());
            for alt in alts {
                patterns.push(sequence_to_pattern(name, &alt.items)?);
            }
            if patterns.len() == 1 {
                return Ok(patterns.pop().unwrap());
            }
            let mut keyed = Vec::with_capacity(patterns.len());
            for pattern in patterns {
                let (min, max) = pattern_widths(name, &pattern)?;
                keyed.push((
                    max.unwrap_or(u64::MAX),
                    min,
                    pattern.value.chars().count(),
                    pattern,
                ));
            }
            keyed.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)));
            let joined: Vec<String> = keyed.iter().map(|k| pattern_to_regexp(&k.3)).collect();
            Ok(Pattern::re(format!("(?:{})", joined.join("|"))))
        }
        Expr::Literal(pattern) => Ok(pattern.clone()),
        Expr::Repeat { inner, min, max } => {
            let inner = terminal_tree_to_pattern(name, inner)?;
            Ok(Pattern::re(format!(
                "(?:{}){}",
                pattern_to_regexp(&inner),
                repeat_suffix(*min, *max)
            ))
            .with_flags(&inner.flags))
        }
        Expr::Symbol(_) | Expr::Template { .. } => {
            unreachable!("terminal bodies have their symbols inlined and templates rejected")
        }
    }
}

fn sequence_to_pattern(name: &str, items: &[Expr]) -> GrammarResult<Pattern> {
    match items {
        [] => Ok(Pattern::str("")),
        [item] => terminal_tree_to_pattern(name, item),
        _ => {
            let mut joined = String::new();
            for item in items {
                joined.push_str(&pattern_to_regexp(&terminal_tree_to_pattern(name, item)?));
            }
            Ok(Pattern::re(joined))
        }
    }
}

fn contains_template(expr: &Expr) -> bool {
    match expr {
        Expr::Alternatives(alts) => alts.iter().any(|a| a.items.iter().any(contains_template)),
        Expr::Symbol(_) | Expr::Literal(_) => false,
        Expr::Repeat { inner, .. } => contains_template(inner),
        Expr::Template { .. } => true,
    }
}

struct Compiler {
    terminals: Vec<TerminalDef>,
    term_ids: HashMap<String, TermId>,
    /// The named terminal a literal in a rule stands for, by pattern: the
    /// last one defined with that pattern, as in Lark's `term_reverse`.
    term_by_pattern: HashMap<Pattern, TermId>,
    anon_counter: usize,
    rules: Vec<Option<Rule>>,
    rule_ids: HashMap<String, RuleId>,
    templates: HashMap<String, (Vec<String>, Expr)>,
    pending: Vec<(String, Expr)>,
}

impl Compiler {
    fn rule_id(&mut self, name: &str) -> RuleId {
        if let Some(&id) = self.rule_ids.get(name) {
            return id;
        }
        let id = self.rules.len();
        self.rules.push(None);
        self.rule_ids.insert(name.to_string(), id);
        id
    }

    fn add_terminal(
        &mut self,
        name: String,
        pattern: Option<Pattern>,
        priority: i64,
    ) -> GrammarResult<TermId> {
        let term = TerminalDef::new(name, pattern, priority)?;
        let id = self.terminals.len();
        if let Some(p) = &term.pattern {
            self.term_by_pattern.insert(p.clone(), id);
        }
        self.term_ids.insert(term.name.clone(), id);
        self.terminals.push(term);
        Ok(id)
    }

    /// Lark's `PrepareAnonTerminals.pattern`: the terminal for a literal
    /// used inside a rule, created and named on first use.
    fn anonymous_terminal(&mut self, pattern: &Pattern) -> GrammarResult<TermId> {
        if let Some(&id) = self.term_by_pattern.get(pattern) {
            return Ok(id);
        }
        let mut name = None;
        if pattern.kind == PatternKind::Str {
            let candidate = match punctuation_name(&pattern.value) {
                Some(n) => Some(n.to_string()),
                None if is_identifier(&pattern.value) => {
                    let upper = pattern.value.to_uppercase();
                    (!self.term_ids.contains_key(&upper)).then_some(upper)
                }
                None => None,
            };
            if let Some(candidate) = candidate {
                if !self.term_ids.contains_key(&candidate) {
                    name = Some(candidate);
                }
            }
        }
        let name = match name {
            Some(n) => n,
            None => {
                let n = format!("__ANON_{}", self.anon_counter);
                self.anon_counter += 1;
                n
            }
        };
        self.add_terminal(name, Some(pattern.clone()), 0)
    }

    fn symbol_name(&self, item: &Item) -> String {
        match item {
            Item::Terminal(id) => self.terminals[*id].name.clone(),
            Item::Rule(id) => self
                .rule_ids
                .iter()
                .find(|(_, v)| **v == *id)
                .map(|(k, _)| k.clone())
                .unwrap(),
            Item::Group(_) | Item::Repeat { .. } => {
                unreachable!("template arguments are always symbols")
            }
        }
    }

    /// Lark's `ApplyTemplates.template_usage`: the rule instantiating
    /// `name` with `args`, created on first use.
    fn instantiate_template(
        &mut self,
        rule_name: &str,
        name: &str,
        args: &[Expr],
    ) -> GrammarResult<RuleId> {
        let mut arg_items = Vec::with_capacity(args.len());
        for arg in args {
            arg_items.push(self.prepare(rule_name, arg)?);
        }
        let arg_names: Vec<String> = arg_items.iter().map(|a| self.symbol_name(a)).collect();
        let result_name = format!("{name}{{{}}}", arg_names.join(","));
        if let Some(&id) = self.rule_ids.get(&result_name) {
            return Ok(id);
        }
        let Some((params, body)) = self.templates.get(name).cloned() else {
            return Err(GrammarError::new(format!(
                "Template '{name}' used but not defined (in rule {rule_name})"
            )));
        };
        if result_name.matches('{').count() > MAX_TEMPLATE_NESTING {
            return Err(GrammarError::new(format!(
                "Template '{name}' nests its instantiations more than {MAX_TEMPLATE_NESTING} levels deep (in rule {rule_name})"
            )));
        }
        let substitutions: HashMap<&str, Symbol> = params
            .iter()
            .map(|p| p.as_str())
            .zip(arg_items.iter().map(|a| match a {
                Item::Terminal(id) => Symbol::Terminal(self.terminals[*id].name.clone()),
                Item::Rule(_) => Symbol::Rule(self.symbol_name(a)),
                _ => unreachable!("template arguments are always symbols"),
            }))
            .collect();
        let mut body = body;
        substitute(&mut body, &substitutions);
        let id = self.rule_id(&result_name);
        self.pending.push((result_name, body));
        Ok(id)
    }

    fn prepare(&mut self, rule_name: &str, expr: &Expr) -> GrammarResult<Item> {
        Ok(match expr {
            Expr::Alternatives(alts) => {
                let mut sequences = Vec::with_capacity(alts.len());
                for alt in alts {
                    if alt.alias.is_some() {
                        return Err(GrammarError::new(format!(
                            "Deep aliasing not allowed (in rule {rule_name}): aliases may only name a rule's top-level alternatives"
                        )));
                    }
                    sequences.push(self.prepare_sequence(rule_name, &alt.items)?);
                }
                Item::Group(sequences)
            }
            Expr::Symbol(Symbol::Terminal(name)) => match self.term_ids.get(name) {
                Some(&id) => Item::Terminal(id),
                None => unreachable!("undefined terminals are rejected before compiling"),
            },
            Expr::Symbol(Symbol::Rule(name)) => Item::Rule(self.rule_id(name)),
            Expr::Literal(pattern) => Item::Terminal(self.anonymous_terminal(pattern)?),
            Expr::Repeat { inner, min, max } => Item::Repeat {
                inner: Box::new(self.prepare(rule_name, inner)?),
                min: *min,
                max: *max,
            },
            Expr::Template { name, args } => {
                Item::Rule(self.instantiate_template(rule_name, name, args)?)
            }
        })
    }

    fn prepare_sequence(&mut self, rule_name: &str, items: &[Expr]) -> GrammarResult<Sequence> {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(self.prepare(rule_name, item)?);
        }
        Ok(Sequence { items: out })
    }

    fn compile_rule(&mut self, name: &str, body: &Expr) -> GrammarResult<()> {
        let Expr::Alternatives(alts) = body else {
            unreachable!("rule bodies are always alternatives")
        };
        let mut alternatives = Vec::with_capacity(alts.len());
        for alt in alts {
            if let (Some(alias), true) = (&alt.alias, name.starts_with('_')) {
                return Err(GrammarError::new(format!(
                    "Rule {name} is marked for expansion (it starts with an underscore) and isn't allowed to have aliases (alias={alias})"
                )));
            }
            alternatives.push(self.prepare_sequence(name, &alt.items)?);
        }
        let id = self.rule_id(name);
        self.rules[id] = Some(Rule {
            name: name.to_string(),
            alternatives,
        });
        Ok(())
    }
}

/// Lark's `_ReplaceSymbols`: substitute template parameters in an
/// instantiated template body.
fn substitute(expr: &mut Expr, substitutions: &HashMap<&str, Symbol>) {
    match expr {
        Expr::Alternatives(alts) => {
            for alt in alts {
                for item in &mut alt.items {
                    substitute(item, substitutions);
                }
            }
        }
        Expr::Symbol(Symbol::Rule(name)) => {
            if let Some(replacement) = substitutions.get(name.as_str()) {
                *expr = Expr::Symbol(replacement.clone());
            }
        }
        Expr::Symbol(Symbol::Terminal(_)) | Expr::Literal(_) => {}
        Expr::Repeat { inner, .. } => substitute(inner, substitutions),
        Expr::Template { name, args } => {
            if let Some(replacement) = substitutions.get(name.as_str()) {
                *name = replacement.name().to_string();
            }
            for arg in args {
                substitute(arg, substitutions);
            }
        }
    }
}

fn collect_rule_refs(item: &Item, out: &mut Vec<RuleId>) {
    match item {
        Item::Terminal(_) => {}
        Item::Rule(id) => out.push(*id),
        Item::Group(seqs) => {
            for seq in seqs {
                for item in &seq.items {
                    collect_rule_refs(item, out);
                }
            }
        }
        Item::Repeat { inner, .. } => collect_rule_refs(inner, out),
    }
}

fn collect_term_refs(item: &Item, out: &mut Vec<TermId>) {
    match item {
        Item::Terminal(id) => out.push(*id),
        Item::Rule(_) => {}
        Item::Group(seqs) => {
            for seq in seqs {
                for item in &seq.items {
                    collect_term_refs(item, out);
                }
            }
        }
        Item::Repeat { inner, .. } => collect_term_refs(inner, out),
    }
}

fn renumber(
    item: &mut Item,
    rule_map: &HashMap<RuleId, RuleId>,
    term_map: &HashMap<TermId, TermId>,
) {
    match item {
        Item::Terminal(id) => *id = term_map[id],
        Item::Rule(id) => *id = rule_map[id],
        Item::Group(seqs) => {
            for seq in seqs {
                for item in &mut seq.items {
                    renumber(item, rule_map, term_map);
                }
            }
        }
        Item::Repeat { inner, .. } => renumber(inner, rule_map, term_map),
    }
}

/// Compile a built grammar for the given start rules.
pub fn compile(built: &BuiltGrammar, start: &[&str]) -> GrammarResult<Grammar> {
    let mut compiler = Compiler {
        terminals: Vec::new(),
        term_ids: HashMap::default(),
        term_by_pattern: HashMap::default(),
        anon_counter: 0,
        rules: Vec::new(),
        rule_ids: HashMap::default(),
        templates: HashMap::default(),
        pending: Vec::new(),
    };
    for (name, body, priority) in &built.term_defs {
        let pattern = match body {
            Some(body) => {
                if contains_template(body) {
                    return Err(GrammarError::new(format!(
                        "Templates cannot be used inside terminals ({name})"
                    )));
                }
                if body.is_single_empty() {
                    return Err(GrammarError::new(format!(
                        "Terminals cannot be empty ({name})"
                    )));
                }
                Some(terminal_tree_to_pattern(name, body)?)
            }
            None => None,
        };
        compiler.add_terminal(name.clone(), pattern, *priority)?;
    }
    for (name, params, body, _) in &built.rule_defs {
        if params.is_empty() {
            compiler.pending.push((name.clone(), body.clone()));
        } else {
            compiler
                .templates
                .insert(name.clone(), (params.clone(), body.clone()));
        }
    }
    let mut next = 0;
    while next < compiler.pending.len() {
        let (name, body) = compiler.pending[next].clone();
        compiler.compile_rule(&name, &body)?;
        next += 1;
    }
    for (name, &id) in &compiler.rule_ids {
        if compiler.rules[id].is_none() {
            return Err(GrammarError::new(format!(
                "Template '{name}' is used without arguments"
            )));
        }
    }
    let rules: Vec<Rule> = compiler.rules.into_iter().map(|r| r.unwrap()).collect();
    let mut start_ids = Vec::with_capacity(start.len());
    for s in start {
        match compiler.rule_ids.get(*s) {
            Some(&id) => start_ids.push(id),
            None => {
                return Err(GrammarError::new(format!(
                    "Start rule '{s}' is not defined in the grammar"
                )));
            }
        }
    }
    let mut reachable: HashSet<RuleId> = HashSet::default();
    let mut queue = start_ids.clone();
    while let Some(id) = queue.pop() {
        if !reachable.insert(id) {
            continue;
        }
        let mut refs = Vec::new();
        for seq in &rules[id].alternatives {
            for item in &seq.items {
                collect_rule_refs(item, &mut refs);
            }
        }
        queue.extend(refs);
    }
    let mut used_terms: HashSet<TermId> = HashSet::default();
    for (id, rule) in rules.iter().enumerate() {
        if !reachable.contains(&id) {
            continue;
        }
        let mut refs = Vec::new();
        for seq in &rule.alternatives {
            for item in &seq.items {
                collect_term_refs(item, &mut refs);
            }
        }
        used_terms.extend(refs);
    }
    let mut ignore = Vec::with_capacity(built.ignore.len());
    for name in &built.ignore {
        let id = compiler.term_ids[name];
        used_terms.insert(id);
        ignore.push(id);
    }
    let mut rule_map: HashMap<RuleId, RuleId> = HashMap::default();
    let mut kept_rules = Vec::new();
    for (id, rule) in rules.into_iter().enumerate() {
        if reachable.contains(&id) {
            rule_map.insert(id, kept_rules.len());
            kept_rules.push(rule);
        }
    }
    let mut term_map: HashMap<TermId, TermId> = HashMap::default();
    let mut kept_terms = Vec::new();
    for (id, term) in compiler.terminals.into_iter().enumerate() {
        if used_terms.contains(&id) {
            term_map.insert(id, kept_terms.len());
            kept_terms.push(term);
        }
    }
    for rule in &mut kept_rules {
        for seq in &mut rule.alternatives {
            for item in &mut seq.items {
                renumber(item, &rule_map, &term_map);
            }
        }
    }
    Ok(Grammar {
        rules: kept_rules,
        terminals: kept_terms,
        ignore: ignore.into_iter().map(|id| term_map[&id]).collect(),
        start: start_ids.into_iter().map(|id| rule_map[&id]).collect(),
    })
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/compile_tests.rs"]
mod tests;
