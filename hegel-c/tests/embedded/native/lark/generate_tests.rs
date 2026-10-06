use super::*;
use crate::native::lark::builder::ImportSource;
use crate::native::rng::EngineRng;
use alloc::string::ToString;
use alloc::vec;
use std::path::Path;

struct FixtureImports;

impl ImportSource for FixtureImports {
    fn load(&self, base_path: Option<&str>, grammar_path: &str) -> Option<(String, String)> {
        let base = base_path?;
        let path = Path::new(base).join(grammar_path);
        let text = std::fs::read_to_string(&path).ok()?;
        Some((path.to_str().unwrap().to_string(), text))
    }
}

fn fixture_spec(
    name: &str,
    start: &str,
    max_size: u64,
    explicit: Vec<(String, StringSpec)>,
) -> GrammarSpec {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lark")
        .join(format!("{name}.lark"));
    let text = std::fs::read_to_string(&path).unwrap();
    let built = load_grammar(&text, Some(path.to_str().unwrap()), &FixtureImports).unwrap();
    let grammar = compile(&built, &[start]).unwrap();
    GrammarSpec::new(
        grammar,
        GrammarOptions {
            start: start.to_string(),
            max_size,
            explicit,
            ..GrammarOptions::default()
        },
    )
    .unwrap()
}

#[test]
#[ignore]
fn experiment() {
    let name = std::env::var("LARK_GRAMMAR").unwrap_or_else(|_| "json".to_string());
    let start = std::env::var("LARK_START").unwrap_or_else(|_| "start".to_string());
    let n: usize = std::env::var("LARK_N")
        .map(|s| s.parse().unwrap())
        .unwrap_or(200);
    let max_size: u64 = std::env::var("LARK_MAX")
        .map(|s| s.parse().unwrap())
        .unwrap_or(DEFAULT_MAX_SIZE);
    let out = std::env::var("LARK_OUT").unwrap();
    let explicit = std::env::var("LARK_EXPLICIT")
        .map(|v| {
            v.split(',')
                .map(|pair| {
                    let (name, text) = pair.split_once('=').unwrap();
                    let text = text.replace("\\n", "\n");
                    let pattern = crate::native::lark::escape::re_escape(&text);
                    (
                        name.to_string(),
                        StringSpec::regex(&pattern, true, None).unwrap(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let spec = fixture_spec(&name, &start, max_size, explicit);
    let mut sizes = vec![];
    let mut invalid = 0;
    let mut lines = String::new();
    let started = std::time::Instant::now();
    for seed in 0..n as u64 {
        let mut ntc = NativeTestCase::new_random(EngineRng::seeded(seed)).unwrap();
        match generate_grammar(&mut ntc, &spec) {
            Ok(s) => {
                sizes.push(s.chars().count());
                lines.push_str(&serde_json::to_string(&s).unwrap());
                lines.push('\n');
            }
            Err(e) => {
                invalid += 1;
                std::eprintln!("seed {seed}: {e:?} status {:?}", ntc.status());
            }
        }
    }
    let elapsed = started.elapsed();
    std::fs::write(&out, lines).unwrap();
    sizes.sort_unstable();
    let pct = |p: f64| sizes[((sizes.len() - 1) as f64 * p) as usize];
    std::eprintln!(
        "{name}: n={} invalid={invalid} min={} p10={} p25={} p50={} p75={} p90={} max={} mean={:.1} time/case={:?}",
        sizes.len(),
        sizes[0],
        pct(0.1),
        pct(0.25),
        pct(0.5),
        pct(0.75),
        pct(0.9),
        sizes[sizes.len() - 1],
        sizes.iter().sum::<usize>() as f64 / sizes.len() as f64,
        elapsed / n as u32
    );
}
