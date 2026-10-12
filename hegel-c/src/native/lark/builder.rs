//! Assembling a grammar from its statements: Lark's `GrammarBuilder`.
//! Handles `%import` (with Lark's name mangling and pruning of the
//! imported grammar to what was asked for), `%override`, `%extend`,
//! `%ignore`, `%declare`, template parameters, terminal references inside
//! terminals, and the validation Lark performs before compiling.

use super::ast::{
    Definition as DefStmt, Expr, ImportNames, ImportStmt, RuleModifiers, Statement, Symbol,
};
use super::parser::parse_grammar;
use super::{GrammarError, GrammarResult};
use crate::native::{HashMap, HashSet};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Where `%import`ed grammars come from: a lookup of `grammar_path`
/// (a relative file path such as `common.lark` or `dir/sub.lark`) given
/// the directory of the importing grammar when that is known. Returns the
/// path the grammar was found at and its text.
pub trait ImportSource {
    fn load(&self, base_path: Option<&str>, grammar_path: &str) -> Option<(String, String)>;
}

/// An import source with no grammars: only the bundled ones resolve.
pub struct NoImports;

impl ImportSource for NoImports {
    fn load(&self, _base_path: Option<&str>, _grammar_path: &str) -> Option<(String, String)> {
        None
    }
}

/// The grammars Lark 1.3.1 ships in `lark/grammars/`, copied verbatim
/// under `grammars/` with Lark's licence, reachable as `%import common.X`
/// and so on.
pub fn bundled_grammar(grammar_path: &str) -> Option<&'static str> {
    match grammar_path {
        "common.lark" => Some(include_str!("grammars/common.lark")),
        "lark.lark" => Some(include_str!("grammars/lark.lark")),
        "python.lark" => Some(include_str!("grammars/python.lark")),
        "unicode.lark" => Some(include_str!("grammars/unicode.lark")),
        _ => None,
    }
}

/// Options recorded on a rule definition. Only `priority` and the
/// template parameters affect anything here; the tree-shaping options are
/// kept so the loader accepts every grammar Lark does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleOptions {
    pub modifiers: RuleModifiers,
    pub priority: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefOptions {
    Rule(RuleOptions),
    Terminal { priority: i64 },
}

/// A rule or terminal definition as the builder holds it: `tree` is
/// `None` for a `%declare`d terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub is_term: bool,
    pub params: Vec<String>,
    pub tree: Option<Expr>,
    pub options: DefOptions,
}

/// Lark's import-time renaming: `X` imported from `a.b` becomes `a__b__X`
/// (`_X` becomes `_a__b__X`), except for the names the import asked for,
/// which take their alias; a nested import composes with the importer's
/// own mangle.
pub struct Mangle {
    prefix: String,
    aliases: Vec<(String, String)>,
    base: Option<Box<Mangle>>,
}

impl Mangle {
    pub fn apply(&self, s: &str) -> String {
        let mut out = match self.aliases.iter().find(|(from, _)| from == s) {
            Some((_, to)) => to.clone(),
            None => match s.strip_prefix('_') {
                Some(rest) => format!("_{}__{rest}", self.prefix),
                None => format!("{}__{s}", self.prefix),
            },
        };
        if let Some(base) = &self.base {
            out = base.apply(&out);
        }
        out
    }

    fn clone_boxed(&self) -> Box<Mangle> {
        Box::new(Mangle {
            prefix: self.prefix.clone(),
            aliases: self.aliases.clone(),
            base: self.base.as_ref().map(|b| b.clone_boxed()),
        })
    }
}

/// The grammar as built: Lark's `Grammar(rule_defs, term_defs, ignore)`,
/// ready for [`super::compile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltGrammar {
    /// `(name, params, body, options)` in definition order.
    pub rule_defs: Vec<(String, Vec<String>, Expr, RuleOptions)>,
    /// `(name, body, priority)` in definition order; `body` is `None` for a
    /// `%declare`d terminal.
    pub term_defs: Vec<(String, Option<Expr>, i64)>,
    pub ignore: Vec<String>,
}

pub struct GrammarBuilder<'s> {
    source: &'s dyn ImportSource,
    names: Vec<String>,
    definitions: HashMap<String, Definition>,
    ignore_names: Vec<String>,
}

fn type_name(is_term: bool) -> &'static str {
    if is_term { "terminal" } else { "rule" }
}

fn type_title(is_term: bool) -> &'static str {
    if is_term { "Terminal" } else { "Rule" }
}

fn mangle_expr(expr: &mut Expr, mangle: &Mangle) {
    match expr {
        Expr::Alternatives(alts) => {
            for alt in alts {
                for item in &mut alt.items {
                    mangle_expr(item, mangle);
                }
            }
        }
        Expr::Symbol(Symbol::Terminal(name) | Symbol::Rule(name)) => {
            *name = mangle.apply(name);
        }
        Expr::Literal(_) => {}
        Expr::Repeat { inner, .. } => mangle_expr(inner, mangle),
        Expr::Template { name, args } => {
            *name = mangle.apply(name);
            for arg in args {
                mangle_expr(arg, mangle);
            }
        }
    }
}

fn collect_symbols<'e>(expr: &'e Expr, out: &mut Vec<&'e str>) {
    match expr {
        Expr::Alternatives(alts) => {
            for alt in alts {
                for item in &alt.items {
                    collect_symbols(item, out);
                }
            }
        }
        Expr::Symbol(symbol) => out.push(symbol.name()),
        Expr::Literal(_) => {}
        Expr::Repeat { inner, .. } => collect_symbols(inner, out),
        Expr::Template { name, args } => {
            out.push(name);
            for arg in args {
                collect_symbols(arg, out);
            }
        }
    }
}

/// Every symbol name referenced by `expr`, including template names.
pub fn find_used_symbols(expr: &Expr) -> Vec<&str> {
    let mut out = Vec::new();
    collect_symbols(expr, &mut out);
    out
}

fn collect_templates<'e>(expr: &'e Expr, out: &mut Vec<(&'e str, usize)>) {
    match expr {
        Expr::Alternatives(alts) => {
            for alt in alts {
                for item in &alt.items {
                    collect_templates(item, out);
                }
            }
        }
        Expr::Symbol(_) | Expr::Literal(_) => {}
        Expr::Repeat { inner, .. } => collect_templates(inner, out),
        Expr::Template { name, args } => {
            out.push((name, args.len()));
            for arg in args {
                collect_templates(arg, out);
            }
        }
    }
}

/// The symbols an expression references directly (not through templates).
fn symbol_references<'e>(expr: &'e Expr, out: &mut Vec<&'e Symbol>) {
    match expr {
        Expr::Alternatives(alts) => {
            for alt in alts {
                for item in &alt.items {
                    symbol_references(item, out);
                }
            }
        }
        Expr::Symbol(symbol) => out.push(symbol),
        Expr::Literal(_) | Expr::Template { .. } => {}
        Expr::Repeat { inner, .. } => symbol_references(inner, out),
    }
}

/// Replace every terminal reference in `expr` with the referenced
/// terminal's body, as Lark's `resolve_term_references` does. Bodies are
/// taken from `resolved`, which the caller fills in dependency order.
fn inline_terminals(expr: &mut Expr, resolved: &HashMap<String, Expr>) {
    match expr {
        Expr::Alternatives(alts) => {
            for alt in alts {
                for item in &mut alt.items {
                    inline_terminals(item, resolved);
                }
            }
        }
        Expr::Symbol(Symbol::Terminal(name)) => {
            *expr = resolved[name.as_str()].clone();
        }
        Expr::Symbol(Symbol::Rule(_)) | Expr::Literal(_) | Expr::Template { .. } => {}
        Expr::Repeat { inner, .. } => inline_terminals(inner, resolved),
    }
}

impl<'s> GrammarBuilder<'s> {
    pub fn new(source: &'s dyn ImportSource) -> Self {
        GrammarBuilder {
            source,
            names: Vec::new(),
            definitions: HashMap::default(),
            ignore_names: Vec::new(),
        }
    }

    fn insert(&mut self, name: String, definition: Definition) {
        if !self.definitions.contains_key(&name) {
            self.names.push(name.clone());
        }
        self.definitions.insert(name, definition);
    }

    fn define(
        &mut self,
        name: String,
        is_term: bool,
        tree: Option<Expr>,
        params: Vec<String>,
        options: DefOptions,
        override_existing: bool,
    ) -> GrammarResult<()> {
        if self.definitions.contains_key(&name) {
            if !override_existing {
                return Err(GrammarError::new(format!(
                    "{} '{name}' defined more than once",
                    type_title(is_term)
                )));
            }
        } else if override_existing {
            return Err(GrammarError::new(format!(
                "Cannot override a nonexisting {} {name}",
                type_name(is_term)
            )));
        }
        self.insert(
            name,
            Definition {
                is_term,
                params,
                tree,
                options,
            },
        );
        Ok(())
    }

    fn extend(
        &mut self,
        name: String,
        is_term: bool,
        tree: Expr,
        params: Vec<String>,
    ) -> GrammarResult<()> {
        let Some(existing) = self.definitions.get_mut(&name) else {
            return Err(GrammarError::new(format!(
                "Can't extend {} {name} as it wasn't defined before",
                type_name(is_term)
            )));
        };
        if existing.params != params {
            return Err(GrammarError::new(format!(
                "Cannot extend {} with different parameters: {name}",
                type_name(is_term)
            )));
        }
        let Some(base) = &mut existing.tree else {
            return Err(GrammarError::new(format!(
                "Can't extend {} {name} - it is abstract.",
                type_name(is_term)
            )));
        };
        let (Expr::Alternatives(alts), Expr::Alternatives(extension)) = (base, tree) else {
            unreachable!("definition bodies are always alternatives")
        };
        alts.splice(0..0, extension);
        Ok(())
    }

    fn ignore(&mut self, expr: Expr) {
        if let Expr::Alternatives(alts) = &expr {
            if alts.len() == 1 && alts[0].items.len() == 1 && alts[0].alias.is_none() {
                if let Expr::Symbol(Symbol::Terminal(name)) = &alts[0].items[0] {
                    self.ignore_names.push(name.clone());
                    return;
                }
            }
        }
        let name = format!("__IGNORE_{}", self.ignore_names.len());
        self.ignore_names.push(name.clone());
        self.insert(
            name,
            Definition {
                is_term: true,
                params: Vec::new(),
                tree: Some(expr),
                options: DefOptions::Terminal { priority: 0 },
            },
        );
    }

    fn declare(&mut self, symbols: &[Symbol], mangle: Option<&Mangle>) -> GrammarResult<()> {
        for symbol in symbols {
            let Symbol::Terminal(name) = symbol else {
                return Err(GrammarError::new(format!(
                    "Expecting terminal name to follow %declare, but got rule name '{}'",
                    symbol.name()
                )));
            };
            let name = match mangle {
                Some(m) => m.apply(name),
                None => name.clone(),
            };
            self.define(
                name,
                true,
                None,
                Vec::new(),
                DefOptions::Terminal { priority: 0 },
                false,
            )?;
        }
        Ok(())
    }

    fn unpack_definition(
        definition: DefStmt,
        mangle: Option<&Mangle>,
    ) -> (String, bool, Expr, Vec<String>, DefOptions) {
        let (name, is_term, mut tree, params, options) = match definition {
            DefStmt::Rule(rule) => (
                rule.name,
                false,
                rule.body,
                rule.params,
                DefOptions::Rule(RuleOptions {
                    modifiers: rule.modifiers,
                    priority: rule.priority,
                }),
            ),
            DefStmt::Terminal(term) => (
                term.name,
                true,
                term.body,
                Vec::new(),
                DefOptions::Terminal {
                    priority: term.priority,
                },
            ),
        };
        match mangle {
            Some(m) => {
                mangle_expr(&mut tree, m);
                (
                    m.apply(&name),
                    is_term,
                    tree,
                    params.iter().map(|p| m.apply(p)).collect(),
                    options,
                )
            }
            None => (name, is_term, tree, params, options),
        }
    }

    fn do_import(
        &mut self,
        stmt: &ImportStmt,
        aliases: Vec<(String, String)>,
        base_path: Option<&str>,
        base_mangle: Option<&Mangle>,
    ) -> GrammarResult<()> {
        let mangle = Mangle {
            prefix: stmt.path.join("__"),
            aliases,
            base: base_mangle.map(|m| m.clone_boxed()),
        };
        let grammar_path = format!("{}.lark", stmt.path.join("/"));
        let loaded = self.source.load(base_path, &grammar_path).or_else(|| {
            bundled_grammar(&grammar_path).map(|text| (grammar_path.clone(), text.to_string()))
        });
        let Some((joined_path, text)) = loaded else {
            return Err(GrammarError::new(format!(
                "Could not find grammar `{grammar_path}` to import"
            )));
        };
        let mut imported = GrammarBuilder::new(self.source);
        imported.load_grammar(&text, Some(&joined_path), Some(&mangle))?;
        let used: Vec<String> = mangle
            .aliases
            .iter()
            .map(|(k, _)| mangle.apply(k))
            .collect();
        imported.remove_unused(&used);
        for name in imported.names {
            if self.definitions.contains_key(&name) {
                return Err(GrammarError::new(format!(
                    "Cannot import '{name}' from '{grammar_path}': Symbol already defined."
                )));
            }
            let definition = imported.definitions.remove(&name).unwrap();
            self.insert(name, definition);
        }
        Ok(())
    }

    fn remove_unused(&mut self, used: &[String]) {
        let mut keep: HashSet<String> = HashSet::default();
        let mut queue: Vec<String> = used.to_vec();
        while let Some(name) = queue.pop() {
            if !keep.insert(name.clone()) {
                continue;
            }
            let Some(definition) = self.definitions.get(&name) else {
                continue;
            };
            if definition.is_term {
                continue;
            }
            if let Some(tree) = &definition.tree {
                for symbol in find_used_symbols(tree) {
                    if !definition.params.iter().any(|p| p == symbol) {
                        queue.push(symbol.to_string());
                    }
                }
            }
        }
        self.names.retain(|n| keep.contains(n));
        self.definitions.retain(|n, _| keep.contains(n));
    }

    /// Lark's `resolve_term_references`: inline every terminal referenced
    /// from another terminal's body, rejecting rules inside terminals,
    /// undefined terminals and recursive terminals.
    fn resolve_term_references(&mut self) -> GrammarResult<()> {
        let term_names: Vec<String> = self
            .names
            .iter()
            .filter(|n| self.definitions[*n].is_term)
            .cloned()
            .collect();
        for name in &term_names {
            let Some(tree) = &self.definitions[name].tree else {
                continue;
            };
            let mut symbols = Vec::new();
            symbol_references(tree, &mut symbols);
            for symbol in symbols {
                let Symbol::Terminal(reference) = symbol else {
                    return Err(GrammarError::new(format!(
                        "Rules aren't allowed inside terminals ({} in {name})",
                        symbol.name()
                    )));
                };
                match self.definitions.get(reference) {
                    None => {
                        return Err(GrammarError::new(format!(
                            "Terminal used but not defined: {reference}"
                        )));
                    }
                    Some(d) if d.tree.is_none() => {
                        return Err(GrammarError::new(format!(
                            "Terminal {name} uses declared terminal {reference}, which has no definition to inline"
                        )));
                    }
                    Some(_) => {}
                }
            }
        }
        let mut resolved: HashMap<String, Expr> = HashMap::default();
        let mut visiting: Vec<String> = Vec::new();
        for name in &term_names {
            self.resolve_terminal(name, &mut resolved, &mut visiting)?;
        }
        for name in &term_names {
            if let Some(tree) = resolved.remove(name) {
                self.definitions.get_mut(name).unwrap().tree = Some(tree);
            }
        }
        Ok(())
    }

    fn resolve_terminal(
        &self,
        name: &str,
        resolved: &mut HashMap<String, Expr>,
        visiting: &mut Vec<String>,
    ) -> GrammarResult<()> {
        if resolved.contains_key(name) {
            return Ok(());
        }
        let Some(tree) = &self.definitions[name].tree else {
            return Ok(());
        };
        if visiting.iter().any(|v| v == name) {
            return Err(GrammarError::new(format!(
                "Recursion in terminal '{name}' (recursion is only allowed in rules, not terminals)"
            )));
        }
        visiting.push(name.to_string());
        let mut references = Vec::new();
        symbol_references(tree, &mut references);
        for reference in references {
            self.resolve_terminal(reference.name(), resolved, visiting)?;
        }
        visiting.pop();
        let mut tree = tree.clone();
        inline_terminals(&mut tree, resolved);
        resolved.insert(name.to_string(), tree);
        Ok(())
    }

    /// Load one grammar's statements, importing what it imports first.
    /// `source_path` is where the text came from, if it came from a file:
    /// relative imports resolve against its directory. `mangle` is set
    /// when this grammar is itself being imported.
    pub fn load_grammar(
        &mut self,
        text: &str,
        source_path: Option<&str>,
        mangle: Option<&Mangle>,
    ) -> GrammarResult<()> {
        let statements = parse_grammar(text)?;
        let base_path = source_path.map(|p| match p.rfind('/') {
            Some(i) => p[..i].to_string(),
            None => String::new(),
        });
        let mut imports: Vec<(&ImportStmt, Vec<(String, String)>)> = Vec::new();
        for statement in &statements {
            let Statement::Import(stmt) = statement else {
                continue;
            };
            let aliases: Vec<(String, String)> = match &stmt.names {
                ImportNames::Single { name, alias } => {
                    alloc::vec![(name.clone(), alias.clone().unwrap_or_else(|| name.clone()))]
                }
                ImportNames::Multi(names) => names.iter().map(|n| (n.clone(), n.clone())).collect(),
            };
            match imports
                .iter_mut()
                .find(|(s, _)| s.path == stmt.path && s.relative == stmt.relative)
            {
                Some((_, existing)) => {
                    for (from, to) in aliases {
                        match existing.iter_mut().find(|(f, _)| *f == from) {
                            Some(entry) => entry.1 = to,
                            None => existing.push((from, to)),
                        }
                    }
                }
                None => imports.push((stmt, aliases)),
            }
        }
        for (stmt, aliases) in imports {
            let base = if stmt.relative {
                base_path.as_deref()
            } else {
                None
            };
            self.do_import(stmt, aliases, base, mangle)?;
        }
        for statement in statements {
            match statement {
                Statement::Define(definition) => {
                    let (name, is_term, tree, params, options) =
                        Self::unpack_definition(definition, mangle);
                    self.define(name, is_term, Some(tree), params, options, false)?;
                }
                Statement::Override(definition) => {
                    let (name, is_term, tree, params, options) =
                        Self::unpack_definition(definition, mangle);
                    self.define(name, is_term, Some(tree), params, options, true)?;
                }
                Statement::Extend(definition) => {
                    let (name, is_term, tree, params, _) =
                        Self::unpack_definition(definition, mangle);
                    self.extend(name, is_term, tree, params)?;
                }
                Statement::Ignore(expr) => {
                    if mangle.is_none() {
                        self.ignore(expr);
                    }
                }
                Statement::Declare(symbols) => self.declare(&symbols, mangle)?,
                Statement::Import(_) => {}
            }
        }
        self.resolve_term_references()
    }

    /// Lark's `GrammarBuilder.validate`.
    pub fn validate(&self) -> GrammarResult<()> {
        for name in &self.names {
            let d = &self.definitions[name];
            for (i, p) in d.params.iter().enumerate() {
                if self.definitions.contains_key(p) {
                    return Err(GrammarError::new(format!(
                        "Template Parameter conflicts with rule {p} (in template {name})"
                    )));
                }
                if d.params[..i].contains(p) {
                    return Err(GrammarError::new(format!(
                        "Duplicate Template Parameter {p} (in template {name})"
                    )));
                }
            }
            let Some(tree) = &d.tree else {
                continue;
            };
            let mut templates = Vec::new();
            collect_templates(tree, &mut templates);
            for (sym, arity) in templates {
                if d.params.iter().any(|p| p == sym) {
                    continue;
                }
                let Some(target) = self.definitions.get(sym) else {
                    return Err(GrammarError::new(format!(
                        "Template '{sym}' used but not defined (in {} {name})",
                        type_name(d.is_term)
                    )));
                };
                if arity != target.params.len() {
                    return Err(GrammarError::new(format!(
                        "Wrong number of template arguments used for {sym} (expected {}, got {arity}) (in {} {name})",
                        target.params.len(),
                        type_name(d.is_term)
                    )));
                }
            }
            for sym in find_used_symbols(tree) {
                if !self.definitions.contains_key(sym) && !d.params.iter().any(|p| p == sym) {
                    let sym_is_term = tree_mentions_terminal(tree, sym);
                    return Err(GrammarError::new(format!(
                        "{} '{sym}' used but not defined (in {} {name})",
                        type_title(sym_is_term),
                        type_name(d.is_term)
                    )));
                }
            }
        }
        let undefined: Vec<&String> = self
            .ignore_names
            .iter()
            .filter(|n| !self.definitions.contains_key(*n))
            .collect();
        if !undefined.is_empty() {
            let names: Vec<String> = undefined.iter().map(|n| format!("'{n}'")).collect();
            return Err(GrammarError::new(format!(
                "Terminals {{{}}} were marked to ignore but were not defined!",
                names.join(", ")
            )));
        }
        Ok(())
    }

    pub fn build(self) -> GrammarResult<BuiltGrammar> {
        self.validate()?;
        let mut rule_defs = Vec::new();
        let mut term_defs = Vec::new();
        let mut definitions = self.definitions;
        for name in self.names {
            let d = definitions.remove(&name).unwrap();
            match d.options {
                DefOptions::Terminal { priority } => term_defs.push((name, d.tree, priority)),
                DefOptions::Rule(options) => {
                    rule_defs.push((name, d.params, d.tree.unwrap(), options));
                }
            }
        }
        Ok(BuiltGrammar {
            rule_defs,
            term_defs,
            ignore: self.ignore_names,
        })
    }
}

fn tree_mentions_terminal(tree: &Expr, name: &str) -> bool {
    tree_mentions(tree, name, true)
}

fn tree_mentions(tree: &Expr, name: &str, as_terminal: bool) -> bool {
    match tree {
        Expr::Alternatives(alts) => alts.iter().any(|alt| {
            alt.items
                .iter()
                .any(|i| tree_mentions(i, name, as_terminal))
        }),
        Expr::Symbol(symbol) => symbol.name() == name && symbol.is_terminal() == as_terminal,
        Expr::Literal(_) => false,
        Expr::Repeat { inner, .. } => tree_mentions(inner, name, as_terminal),
        Expr::Template { name: t, args } => {
            (!as_terminal && t == name) || args.iter().any(|a| tree_mentions(a, name, as_terminal))
        }
    }
}

/// Parse and build a grammar from its text.
pub fn load_grammar(
    text: &str,
    source_path: Option<&str>,
    source: &dyn ImportSource,
) -> GrammarResult<BuiltGrammar> {
    let mut builder = GrammarBuilder::new(source);
    builder.load_grammar(text, source_path, None)?;
    builder.build()
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/lark/builder_tests.rs"]
mod tests;
