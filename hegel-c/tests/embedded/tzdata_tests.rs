use super::*;
use jiff::Timestamp;
use jiff::civil::{Date, date};
use jiff::tz::TimeZone;
use std::collections::BTreeSet;
use std::format;
use std::path::Path;
use std::process::Command;
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;

const TABLE_TEXT: &str = include_str!("../../src/tzdata/tzdata.txt");
const SCRIPT: &str = include_str!("../../../scripts/generate_tzdata.py");

fn all_names() -> &'static [Name] {
    names().unwrap()
}

fn jiff_zone(name: &str) -> TimeZone {
    TimeZone::get(name).unwrap_or_else(|e| panic!("jiff has no zone {name}: {e}"))
}

fn at(second: i64) -> Timestamp {
    Timestamp::from_second(second).unwrap()
}

fn utc_date(second: i64) -> Date {
    at(second).to_zoned(TimeZone::UTC).date()
}

fn script_constant(name: &str) -> &'static str {
    let prefix = format!("{name} = \"");
    let line = SCRIPT
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("{name} is not set in generate_tzdata.py"));
    line[prefix.len()..].strip_suffix('"').unwrap()
}

#[test]
fn version_matches_the_script_and_jiff() {
    assert_eq!(VERSION, script_constant("IANA_VERSION"));
    assert_eq!(jiff_tzdb::VERSION, Some(VERSION));
    assert_eq!(VERSION_CSTR.to_str().unwrap(), VERSION);
    assert_eq!(table().unwrap().version, VERSION);
    assert!(TABLE_TEXT.contains(&format!("\nV {VERSION}\n")));
    let pin = format!("tzdata=={}", script_constant("TZDATA_PYPI_VERSION"));
    assert!(
        SCRIPT.contains(&pin),
        "the inline script metadata must pin {pin}"
    );
}

#[test]
fn names_cover_the_same_set_as_jiff() {
    let ours: BTreeSet<&str> = all_names().iter().map(|n| n.name.as_str()).collect();
    let theirs: BTreeSet<String> = jiff::tz::db().available().map(|n| n.to_string()).collect();
    let theirs: BTreeSet<&str> = theirs.iter().map(String::as_str).collect();
    assert_eq!(ours, theirs);
    assert_eq!(all_names().len(), 598);
}

#[test]
fn explicit_transitions_agree_with_jiff() {
    for entry in all_names() {
        let name = entry.name.as_str();
        let tz = jiff_zone(name);
        let zone = entry.zone;
        let mut previous = zone.initial;
        let mut ours = Vec::new();
        for t in &zone.transitions {
            let context = format!("{name} at {}", t.instant);
            assert_eq!(
                tz.to_offset(at(t.instant)).seconds(),
                t.kind.utoff,
                "{context}"
            );
            assert_eq!(
                tz.to_offset(at(t.instant - 1)).seconds(),
                previous.utoff,
                "{context} (before)"
            );
            if t.kind.utoff != previous.utoff {
                let next = tz.following(at(t.instant - 1)).next().unwrap();
                assert_eq!(next.timestamp().as_second(), t.instant, "{context}");
                assert_eq!(next.offset().seconds(), t.kind.utoff, "{context}");
                ours.push(t.instant);
            }
            previous = t.kind;
        }
        let last = zone.transitions.last().map_or(i64::MIN, |t| t.instant);
        let theirs: Vec<i64> = tz
            .following(Timestamp::MIN)
            .map(|tr| tr.timestamp().as_second())
            .take_while(|&instant| instant <= last)
            .filter(|&instant| {
                tz.to_offset(at(instant)).seconds() != tz.to_offset(at(instant - 1)).seconds()
            })
            .collect();
        assert_eq!(ours, theirs, "{name}");
        let initial = zone.transitions.first().map_or(0, |t| t.instant - 1);
        assert_eq!(
            tz.to_offset(at(initial)).seconds(),
            zone.initial.utoff,
            "{name}"
        );
    }
}

fn check_rule_date(rule: RuleDate, wall: i64, context: &str) {
    let midnight = wall - i64::from(rule.time);
    assert_eq!(midnight.rem_euclid(86400), 0, "{context}");
    let day = utc_date(midnight);
    match rule.day {
        RuleDay::MonthWeekDay {
            month,
            week,
            weekday,
        } => {
            assert_eq!(day.month(), i8::try_from(month).unwrap(), "{context}");
            assert_eq!(
                day.weekday().to_sunday_zero_offset(),
                i8::try_from(weekday).unwrap(),
                "{context}"
            );
            if week == 5 {
                assert!(day.day() + 7 > day.days_in_month(), "{context}");
            } else {
                assert_eq!(
                    (day.day() - 1) / 7 + 1,
                    i8::try_from(week).unwrap(),
                    "{context}"
                );
            }
        }
        RuleDay::Julian(n) => {
            assert_eq!(
                day.day_of_year_no_leap(),
                Some(i16::try_from(n).unwrap()),
                "{context}"
            );
        }
        RuleDay::ZeroBased(n) => {
            assert_eq!(
                day.day_of_year() - 1,
                i16::try_from(n).unwrap(),
                "{context}"
            );
        }
    }
}

#[test]
fn footers_agree_with_jiff() {
    for entry in all_names() {
        let name = entry.name.as_str();
        let tz = jiff_zone(name);
        let zone = entry.zone;
        let footer = zone.footer.unwrap();
        let last = zone.transitions.last().map_or(0, |t| t.instant);
        let year = utc_date(last).year() + 2;
        let from = date(year, 1, 1)
            .to_zoned(TimeZone::UTC)
            .unwrap()
            .timestamp();
        let to = date(year + 3, 1, 1)
            .to_zoned(TimeZone::UTC)
            .unwrap()
            .timestamp();
        let transitions: Vec<_> = tz
            .following(from)
            .take_while(|t| t.timestamp() < to)
            .collect();
        let standard = zone.abbreviations[footer.standard.abbreviation].as_str();
        let Some(dst) = footer.dst else {
            assert!(transitions.is_empty(), "{name}");
            assert_eq!(
                tz.to_offset(from).seconds(),
                footer.standard.utoff,
                "{name}"
            );
            assert_eq!(tz.to_offset_info(from).abbreviation(), standard, "{name}");
            continue;
        };
        assert_eq!(transitions.len(), 6, "{name}");
        for tr in &transitions {
            let context = format!("{name} at {}", tr.timestamp());
            let after = tr.offset().seconds();
            let (rule, before, abbreviation) = if after == dst.kind.utoff {
                (
                    dst.start,
                    footer.standard.utoff,
                    zone.abbreviations[dst.kind.abbreviation].as_str(),
                )
            } else {
                assert_eq!(after, footer.standard.utoff, "{context}");
                (dst.end, dst.kind.utoff, standard)
            };
            assert_eq!(tr.abbreviation(), abbreviation, "{context}");
            check_rule_date(
                rule,
                tr.timestamp().as_second() + i64::from(before),
                &context,
            );
        }
    }
}

#[test]
fn every_link_resolves_to_a_primary_zone() {
    let mut links = 0;
    for entry in all_names() {
        let target = lookup(&entry.zone.name).unwrap().unwrap();
        assert!(
            !target.is_link(),
            "{} resolves to the link {}",
            entry.name,
            target.name
        );
        assert!(core::ptr::eq(target.zone, entry.zone));
        if entry.is_link() {
            links += 1;
            assert_ne!(entry.name, entry.zone.name);
        } else {
            assert_eq!(entry.name, entry.zone.name);
        }
    }
    assert_eq!(links, 253);
}

#[test]
fn lookup_is_an_exact_match() {
    assert_eq!(lookup("UTC").unwrap().unwrap().rank, 0);
    assert!(lookup("utc").unwrap().is_none());
    assert!(lookup("UTC ").unwrap().is_none());
    assert!(lookup("").unwrap().is_none());
    assert!(lookup("Europe/Nowhere").unwrap().is_none());
    let london = lookup("Europe/London").unwrap().unwrap();
    assert_eq!(london.zone.name, "Europe/London");
    assert_eq!(
        lookup("US/Eastern").unwrap().unwrap().zone.name,
        "America/New_York"
    );
}

fn ladder_key(entry: &Name) -> (u8, u32, usize, &str) {
    let name = entry.name.as_str();
    if name == "UTC" {
        (0, 0, 0, name)
    } else if name.starts_with("Etc/") {
        let offset = entry
            .zone
            .footer
            .map_or(0, |f| f.standard.utoff.unsigned_abs());
        (1, offset, 0, name)
    } else if entry.tags.contains(Tag::WholeHour) && entry.tags.contains(Tag::Dst) {
        (2, 0, entry.tags.len(), name)
    } else {
        (3, 0, entry.tags.len(), name)
    }
}

#[test]
fn ranks_are_the_shrink_ladder() {
    let mut by_rank: Vec<&Name> = all_names().iter().collect();
    by_rank.sort_by_key(|n| n.rank);
    for (i, entry) in by_rank.iter().enumerate() {
        assert_eq!(usize::from(entry.rank), i);
    }
    assert_eq!(by_rank[0].name, "UTC");
    assert!(by_rank[1..36].iter().all(|n| n.name.starts_with("Etc/")));
    assert_eq!(
        by_rank[36].tags,
        Tags::EMPTY.with(Tag::WholeHour).with(Tag::Dst)
    );
    let mut by_key: Vec<&Name> = all_names().iter().collect();
    by_key.sort_by_key(|n| ladder_key(n));
    let ranked: Vec<&str> = by_rank.iter().map(|n| n.name.as_str()).collect();
    let keyed: Vec<&str> = by_key.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(ranked, keyed);
}

#[test]
fn appendix_c_zones_carry_their_tags() {
    use Tag::*;
    let expectations: &[(&str, &[Tag], &[Tag])] = &[
        ("UTC", &[Fixed, WholeHour, Link], &[Legacy]),
        ("Etc/UTC", &[Fixed, WholeHour], &[Link, Legacy]),
        ("Etc/GMT", &[Fixed], &[Link]),
        ("GMT", &[Fixed, Link, Legacy], &[]),
        ("Zulu", &[Fixed, Link, Legacy], &[]),
        ("UCT", &[Fixed, Link, Legacy], &[]),
        ("Etc/GMT+5", &[Fixed, InvertedSign], &[Link, Legacy]),
        ("Etc/GMT-14", &[Fixed, InvertedSign], &[Link]),
        ("Factory", &[Fixed, Undefined, Legacy], &[]),
        ("Asia/Calcutta", &[Fixed, Fractional, Link], &[Legacy]),
        ("Asia/Kolkata", &[Fixed, Fractional], &[Link, WholeHour]),
        ("Europe/Kiev", &[Link, Dst], &[Legacy]),
        ("Europe/Kyiv", &[Dst], &[Link]),
        ("US/Eastern", &[Link, Legacy, WholeHour, Dst], &[]),
        ("Asia/Saigon", &[Link], &[Legacy]),
        ("Atlantic/Reykjavik", &[Link, Fixed], &[]),
        ("Europe/Amsterdam", &[Link, Dst], &[]),
        ("EST5EDT", &[Legacy, Dst, WholeHour], &[]),
        ("CST6CDT", &[Legacy, Dst], &[]),
        ("EST", &[Legacy, Fixed], &[]),
        ("MST", &[Legacy, Fixed], &[]),
        ("HST", &[Legacy, Fixed], &[]),
        ("PST8PDT", &[Legacy, Dst], &[]),
        ("America/Argentina/ComodRivadavia", &[Link], &[]),
        ("America/Port-au-Prince", &[Dst], &[]),
        ("Antarctica/DumontDUrville", &[Fixed], &[]),
        ("America/Indiana/Tell_City", &[Dst, BaseChange], &[]),
        (
            "America/New_York",
            &[WholeHour, Dst],
            &[Fractional, Fixed, Link, Legacy, Multi],
        ),
        ("Europe/London", &[WholeHour, Dst, BaseChange], &[]),
        ("Europe/Dublin", &[WholeHour, Dst], &[DstOther]),
        ("Africa/Casablanca", &[Multi, Midnight], &[Dst]),
        ("Asia/Gaza", &[Dst, Multi, Midnight], &[]),
        ("Asia/Hebron", &[Dst, Multi, Midnight], &[]),
        ("America/Nuuk", &[Dst, SpansMidnight, Midnight], &[]),
        ("America/Santiago", &[Dst, Midnight, SpansMidnight], &[]),
        ("Africa/Cairo", &[Dst, Midnight, SpansMidnight, Multi], &[]),
        ("Asia/Beirut", &[Dst, Midnight], &[]),
        ("America/Sao_Paulo", &[Midnight], &[Dst, Fixed]),
        ("Antarctica/Troll", &[DstOther, Undefined], &[Dst]),
        ("Australia/Lord_Howe", &[Fractional, DstOther], &[Dst]),
        ("Pacific/Chatham", &[Fractional, Dst], &[]),
        ("Asia/Kathmandu", &[Fractional, BaseChange], &[]),
        ("Australia/Eucla", &[Fractional], &[WholeHour]),
        ("America/St_Johns", &[Fractional, Dst], &[]),
        ("Asia/Tehran", &[Fractional], &[Dst]),
        ("America/Caracas", &[Fractional, BaseChange], &[]),
        ("Pacific/Norfolk", &[Fractional, Dst, BaseChange], &[]),
        (
            "Asia/Pyongyang",
            &[Fractional, BaseChange, SpansMidnight],
            &[],
        ),
        ("Pacific/Apia", &[WholeDay, BaseChange], &[Dst]),
        ("Pacific/Kwajalein", &[WholeDay, BaseChange], &[]),
        ("Pacific/Kiritimati", &[WholeDay, BaseChange], &[]),
        ("Pacific/Tongatapu", &[WholeHour], &[Fixed, Fractional]),
        ("Europe/Moscow", &[BaseChange], &[Dst, Fixed]),
        ("Europe/Lisbon", &[SameOffset, BaseChange, Dst], &[]),
        (
            "Africa/Monrovia",
            &[Seconds, BaseChange],
            &[WholeHour, Fractional],
        ),
        ("Africa/Juba", &[SpansMidnight, BaseChange], &[]),
        ("Asia/Riyadh", &[Fixed], &[Seconds]),
        ("Asia/Manila", &[WholeHour], &[Fixed]),
        ("Europe/Istanbul", &[BaseChange], &[Dst]),
        ("Europe/Minsk", &[BaseChange], &[Dst]),
        ("Europe/Volgograd", &[BaseChange], &[Dst]),
        ("America/Vancouver", &[BaseChange, SameOffset], &[Dst]),
        ("America/Edmonton", &[BaseChange, SameOffset], &[Dst]),
        ("America/Inuvik", &[BaseChange, SameOffset], &[Dst]),
        ("America/Winnipeg", &[BaseChange, SameOffset], &[Dst]),
    ];
    for &(name, present, absent) in expectations {
        let entry = lookup(name)
            .unwrap()
            .unwrap_or_else(|| panic!("{name} is missing"));
        for &tag in present {
            assert!(entry.tags.contains(tag), "{name} lacks {}", tag.as_str());
        }
        for &tag in absent {
            assert!(!entry.tags.contains(tag), "{name} has {}", tag.as_str());
        }
    }
    let tagged: BTreeSet<Tag> = all_names().iter().flat_map(|n| n.tags.iter()).collect();
    assert_eq!(
        tagged.len(),
        Tag::ALL.len(),
        "every tag is used by some zone"
    );
}

fn footer_of(name: &str) -> Footer {
    lookup(name).unwrap().unwrap().zone.footer.unwrap()
}

#[test]
fn appendix_c_footers_are_parsed_into_fields() {
    let dublin = footer_of("Europe/Dublin");
    assert_eq!(dublin.standard.utoff, 3600);
    assert_eq!(dublin.dst.unwrap().kind.utoff, 0);
    let gaza = footer_of("Asia/Gaza").dst.unwrap();
    assert_eq!(gaza.start.time, 50 * 3600);
    assert_eq!(gaza.end.time, 50 * 3600);
    let nuuk = footer_of("America/Nuuk").dst.unwrap();
    assert_eq!(nuuk.start.time, -3600);
    assert_eq!(
        nuuk.start.day,
        RuleDay::MonthWeekDay {
            month: 3,
            week: 5,
            weekday: 0
        }
    );
    let santiago = footer_of("America/Santiago").dst.unwrap();
    assert_eq!((santiago.start.time, santiago.end.time), (86400, 86400));
    let cairo = footer_of("Africa/Cairo").dst.unwrap();
    assert_eq!(cairo.start.time, 0);
    assert_eq!(
        cairo.end,
        RuleDate {
            day: RuleDay::MonthWeekDay {
                month: 10,
                week: 5,
                weekday: 4
            },
            time: 86400
        }
    );
    let troll = footer_of("Antarctica/Troll");
    assert_eq!(troll.dst.unwrap().kind.utoff - troll.standard.utoff, 7200);
    let lord_howe = footer_of("Australia/Lord_Howe");
    assert_eq!(
        lord_howe.dst.unwrap().kind.utoff - lord_howe.standard.utoff,
        1800
    );
    let chatham = footer_of("Pacific/Chatham");
    assert_eq!(chatham.standard.utoff, 12 * 3600 + 45 * 60);
    let chatham_dst = chatham.dst.unwrap();
    assert_eq!(
        (chatham_dst.start.time, chatham_dst.end.time),
        (9900, 13500)
    );
    let kolkata = lookup("Asia/Kolkata").unwrap().unwrap().zone;
    let kolkata_footer = kolkata.footer.unwrap();
    assert_eq!(kolkata_footer.standard.utoff, 19800);
    assert_eq!(
        kolkata.abbreviations[kolkata_footer.standard.abbreviation],
        "IST"
    );
    assert!(kolkata_footer.dst.is_none());
    let factory = lookup("Factory").unwrap().unwrap().zone;
    assert_eq!(factory.abbreviations, vec!["-00".to_string()]);
    assert_eq!(factory.transitions, Vec::new());
    assert_eq!(factory.initial.utoff, 0);
    let monrovia = lookup("Africa/Monrovia").unwrap().unwrap().zone;
    assert!(
        monrovia
            .transitions
            .iter()
            .any(|t| t.kind.utoff == -(44 * 60 + 30))
    );
}

#[test]
fn tags_round_trip_through_their_names() {
    for tag in Tag::ALL {
        assert_eq!(Tag::from_name(tag.as_str()), Some(tag));
    }
    assert_eq!(Tag::from_name("fixed,dst"), None);
    let tags = Tags::EMPTY.with(Tag::Dst).with(Tag::Link).with(Tag::Dst);
    assert_eq!(tags.len(), 2);
    assert!(!tags.is_empty());
    assert!(Tags::EMPTY.is_empty());
    assert_eq!(tags.iter().collect::<Vec<_>>(), vec![Tag::Dst, Tag::Link]);
    assert!(tags.contains(Tag::Link));
    assert!(!tags.contains(Tag::Fixed));
    assert_eq!(Tags::default(), Tags::EMPTY);
}

#[test]
fn compressed_table_inflates_to_the_text_table() {
    let inflated =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(COMPRESSED_TABLE, MAX_TABLE_LEN)
            .unwrap();
    assert_eq!(String::from_utf8(inflated).unwrap(), TABLE_TEXT);
}

#[test]
fn table_is_reproducible() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/generate_tzdata.py");
    let out = tempfile::tempdir().unwrap();
    let run = Command::new("uv")
        .args(["run", "--script"])
        .arg(&script)
        .arg("--out")
        .arg(out.path())
        .output();
    let output = match run {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            compressed_table_inflates_to_the_text_table();
            return;
        }
        other => other.unwrap(),
    };
    assert!(
        output.status.success(),
        "generate_tzdata.py failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let regenerated = std::fs::read_to_string(out.path().join("tzdata.txt")).unwrap();
    assert_eq!(regenerated, TABLE_TEXT);
    let version = std::fs::read_to_string(out.path().join("version.txt")).unwrap();
    assert_eq!(version, VERSION);
    let compressed = std::fs::read(out.path().join("tzdata.txt.z")).unwrap();
    let inflated = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed).unwrap();
    assert_eq!(String::from_utf8(inflated).unwrap(), TABLE_TEXT);
}

const MINIMAL: &str = "# comment\nV 2026e\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0\n";

fn parse_error(text: &str) -> String {
    parse_table(text)
        .err()
        .unwrap_or_else(|| panic!("parsed: {text:?}"))
        .to_string()
}

#[test]
fn parses_a_minimal_table() {
    let table = parse_table(MINIMAL).unwrap();
    assert_eq!(table.version, "2026e");
    assert_eq!(table.names.len(), 1);
    let utc = &table.names[0];
    assert_eq!(utc.name, "Etc/UTC");
    assert_eq!(utc.tags, Tags::EMPTY.with(Tag::Fixed));
    assert_eq!(utc.zone.footer.unwrap().dst, None);
    assert!(!utc.is_link());
}

#[test]
fn parses_every_rule_day_form() {
    let text = "V x\nZ A/B 1 -\nA S D\nI 0 0 0\nT -5 3600 1 1\nT 7 0 0 0\nF 0 0 1 3600 J60/-7200 59/601200\n\
                L C/D A/B 0 link\n";
    let table = parse_table(text).unwrap();
    let link = &table.names[1];
    assert_eq!(link.name, "C/D");
    assert!(link.is_link());
    assert_eq!(link.zone.name, "A/B");
    assert_eq!(table.names[0].tags, Tags::EMPTY);
    let zone = link.zone;
    assert_eq!(
        zone.transitions,
        vec![
            Transition {
                instant: -5,
                kind: LocalTimeType {
                    utoff: 3600,
                    is_dst: true,
                    abbreviation: 1
                }
            },
            Transition {
                instant: 7,
                kind: LocalTimeType {
                    utoff: 0,
                    is_dst: false,
                    abbreviation: 0
                }
            },
        ]
    );
    let dst = zone.footer.unwrap().dst.unwrap();
    assert_eq!(
        dst.start,
        RuleDate {
            day: RuleDay::Julian(60),
            time: -7200
        }
    );
    assert_eq!(
        dst.end,
        RuleDate {
            day: RuleDay::ZeroBased(59),
            time: 601200
        }
    );
}

#[test]
fn a_zone_without_a_footer_parses() {
    let table = parse_table("V x\nZ A/B 0 -\nA S\nI 60 0 0\n").unwrap();
    assert_eq!(table.names[0].zone.footer, None);
    assert_eq!(table.names[0].zone.initial.utoff, 60);
}

#[test]
fn malformed_tables_are_internal_errors() {
    let cases: &[(&str, &str)] = &[
        ("Z Etc/UTC 0 fixed\nA UTC\nI 0 0 0\n", "no version record"),
        (
            "V a\nV b\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\n",
            "malformed version record",
        ),
        ("V\n", "malformed version record"),
        ("V x\nZ Etc/UTC 0\n", "malformed zone record"),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nL A B 1\n",
            "malformed link record",
        ),
        ("V x\nT 0 0 0 0\n", "T record before any zone"),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nQ 1\n",
            "unknown record \"Q\"",
        ),
        ("V x\nZ Etc/UTC 0 fixed\n", "has no abbreviations"),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nL B Etc/GMT 1 link\n",
            "targets unknown zone Etc/GMT",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nL B Etc/UTC 1 fixed\n",
            "lacks the link tag",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nL Etc/UTC Etc/UTC 1 link\n",
            "listed twice",
        ),
        (
            "V x\nZ Etc/UTC 1 fixed\nA UTC\nI 0 0 0\n",
            "rank 1 beyond the name count",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nL B Etc/UTC 0 link\n",
            "rank 0 is used twice",
        ),
        ("V x\nZ Etc/UTC x fixed\n", "bad number \"x\""),
        ("V x\nZ Etc/UTC 0 fixed,bogus\n", "unknown tag \"bogus\""),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nA UTC\n",
            "malformed abbreviation record",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA\n",
            "malformed abbreviation record",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 2 0\n",
            "malformed local time type",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 1\n",
            "abbreviation index 1 out of range",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nT 5 0 0\n",
            "malformed transition record",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nT 5 0 0 0\nT 5 0 0 0\n",
            "not ascending",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0\n",
            "malformed footer record",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0\nF 0 0\n",
            "malformed footer record",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 M3.2.0 M11.1.0/0\n",
            "has no time",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 M3.2.0/604800 M11.1.0/0\n",
            "rule time 604800 out of range",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 J0/0 M11.1.0/0\n",
            "Julian day 0 out of range",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 M13.1.0/0 M11.1.0/0\n",
            "malformed month rule",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 M3.1/0 M11.1.0/0\n",
            "malformed month rule",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 M3.1.0.0/0 M11.1.0/0\n",
            "malformed month rule",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 M3.x.0/0 M11.1.0/0\n",
            "bad number \"x\"",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nF 0 0 0 3600 366/0 M11.1.0/0\n",
            "day 366 out of range",
        ),
    ];
    for &(text, expected) in cases {
        let message = parse_error(text);
        assert!(message.contains(expected), "{text:?}: {message}");
        assert!(message.contains("bug in hegel"), "{message}");
    }
}
