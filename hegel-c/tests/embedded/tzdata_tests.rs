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
const CHECKSUM: &str = include_str!("../../src/tzdata/checksum.txt");

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
        (1, offset, entry.tags.len(), name)
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
fn utc_is_the_primary_zone() {
    let utc = lookup("UTC").unwrap().unwrap();
    assert!(!utc.is_link());
    assert_eq!(utc.zone.name, "UTC");
    assert_eq!(utc.zone.transitions, Vec::new());
    assert_eq!(utc.zone.initial.utoff, 0);
    for name in [
        "Etc/UTC",
        "Etc/UCT",
        "Etc/Universal",
        "Etc/Zulu",
        "UCT",
        "Universal",
        "Zulu",
    ] {
        let entry = lookup(name).unwrap().unwrap();
        assert!(entry.is_link(), "{name}");
        assert_eq!(entry.zone.name, "UTC", "{name}");
    }
    let lowest_primary = all_names()
        .iter()
        .filter(|n| !n.is_link())
        .min_by_key(|n| n.rank)
        .unwrap();
    assert_eq!(lowest_primary.name, "UTC");
    assert_eq!(TABLE_TEXT.matches("\nZ UTC ").count(), 1);
    assert!(!TABLE_TEXT.contains("\nL UTC "));
}

#[test]
fn name_tags_follow_the_name() {
    const AREAS: [&str; 11] = [
        "Africa",
        "America",
        "Antarctica",
        "Arctic",
        "Asia",
        "Atlantic",
        "Australia",
        "Europe",
        "Indian",
        "Pacific",
        "Etc",
    ];
    for entry in all_names() {
        let name = entry.name.as_str();
        let area = name.split_once('/').map(|(area, _)| area);
        let legacy = name != "UTC" && !area.is_some_and(|area| AREAS.contains(&area));
        assert_eq!(entry.tags.contains(Tag::Legacy), legacy, "{name}");
        let inverted = name
            .strip_prefix("Etc/GMT")
            .and_then(|rest| rest.strip_prefix(['+', '-']))
            .is_some_and(|digits| {
                !digits.is_empty()
                    && !digits.starts_with('0')
                    && digits.bytes().all(|b| b.is_ascii_digit())
            });
        assert_eq!(entry.tags.contains(Tag::InvertedSign), inverted, "{name}");
    }
}

fn year_start(year: i16) -> i64 {
    date(year, 1, 1)
        .to_zoned(TimeZone::UTC)
        .unwrap()
        .timestamp()
        .as_second()
}

fn zone_tags(tags: Tags) -> Tags {
    tags.iter()
        .filter(|tag| ![Tag::Link, Tag::Legacy, Tag::InvertedSign].contains(tag))
        .fold(Tags::EMPTY, Tags::with)
}

type Kind = (i32, String);

fn jiff_tags(entry: &Name) -> Tags {
    let tz = jiff_zone(&entry.name);
    let last = entry.zone.transitions.last().map(|t| t.instant);
    let last_year = last.map_or(1969, |last| utc_date(last).year());
    let end = year_start(last_year.max(1970) + 3);
    let mut changes: Vec<(i64, Kind, Kind)> = Vec::new();
    for tr in tz
        .following(Timestamp::MIN)
        .take_while(|tr| tr.timestamp().as_second() < end)
    {
        let instant = tr.timestamp().as_second();
        let before = tz.to_offset_info(at(instant - 1));
        let before = (before.offset().seconds(), before.abbreviation().to_string());
        let after = (tr.offset().seconds(), tr.abbreviation().to_string());
        if before != after {
            changes.push((instant, before, after));
        }
    }
    let recent: Vec<&(i64, Kind, Kind)> = changes.iter().filter(|c| c.0 >= 0).collect();
    let epoch = tz.to_offset_info(at(0));
    let mut recent_types = vec![(epoch.offset().seconds(), epoch.abbreviation().to_string())];
    recent_types.extend(recent.iter().map(|c| c.2.clone()));
    let mut all_offsets = vec![tz.to_offset(Timestamp::MIN).seconds()];
    all_offsets.extend(changes.iter().map(|c| c.2.0));
    let rule = changes
        .iter()
        .find(|c| last.is_none_or(|last| c.0 > last))
        .map(|c| (c.2.0 - c.1.0).unsigned_abs());
    let offsets_in_year = |year: i16| -> Vec<i32> {
        let (start, end) = (year_start(year), year_start(year + 1));
        let mut offsets = vec![tz.to_offset(at(start)).seconds()];
        offsets.extend(
            changes
                .iter()
                .filter(|c| start < c.0 && c.0 < end)
                .map(|c| c.2.0),
        );
        offsets
    };
    let mut base_change = false;
    let mut previous_min = offsets_in_year(1969).into_iter().min().unwrap();
    for year in 1970..=last_year + 1 {
        let current_min = offsets_in_year(year).into_iter().min().unwrap();
        base_change |= current_min != previous_min;
        previous_min = current_min;
    }
    let mut per_year: std::collections::BTreeMap<i16, usize> = Default::default();
    for c in recent.iter().filter(|c| c.1.0 != c.2.0) {
        *per_year.entry(utc_date(c.0).year()).or_default() += 1;
    }
    let wall = |c: &(i64, Kind, Kind)| (c.0 + i64::from(c.1.0), c.0 + i64::from(c.2.0));
    let conditions = [
        (Tag::Fixed, recent.is_empty()),
        (
            Tag::WholeHour,
            recent_types.iter().all(|(o, _)| o % 3600 == 0),
        ),
        (
            Tag::Fractional,
            recent_types
                .iter()
                .any(|(o, _)| o % 3600 != 0 && o % 60 == 0),
        ),
        (Tag::Seconds, all_offsets.iter().any(|o| o % 60 != 0)),
        (Tag::Dst, rule == Some(3600)),
        (Tag::DstOther, rule.is_some_and(|delta| delta != 3600)),
        (Tag::Multi, per_year.values().any(|&n| n > 2)),
        (Tag::BaseChange, base_change),
        (
            Tag::Midnight,
            recent.iter().any(|c| {
                let (before, after) = wall(c);
                before.rem_euclid(86400) == 0 || after.rem_euclid(86400) == 0
            }),
        ),
        (
            Tag::SpansMidnight,
            recent.iter().any(|c| {
                let (before, after) = wall(c);
                before.div_euclid(86400) != after.div_euclid(86400)
            }),
        ),
        (
            Tag::WholeDay,
            changes
                .iter()
                .any(|c| (c.2.0 - c.1.0).unsigned_abs() >= 86400),
        ),
        (Tag::SameOffset, recent.iter().any(|c| c.1.0 == c.2.0)),
        (
            Tag::Undefined,
            recent_types.iter().any(|(_, abbr)| abbr == "-00"),
        ),
    ];
    conditions
        .into_iter()
        .filter(|&(_, on)| on)
        .fold(Tags::EMPTY, |tags, (tag, _)| tags.with(tag))
}

#[test]
fn zone_tags_agree_with_jiff() {
    for entry in all_names() {
        let primary = lookup(&entry.zone.name).unwrap().unwrap();
        assert_eq!(
            zone_tags(entry.tags),
            zone_tags(primary.tags),
            "{} and {}",
            entry.name,
            primary.name
        );
        if entry.is_link() {
            continue;
        }
        let ours: Vec<Tag> = zone_tags(entry.tags).iter().collect();
        let theirs: Vec<Tag> = jiff_tags(entry).iter().collect();
        assert_eq!(ours, theirs, "{}", entry.name);
    }
}

#[test]
fn appendix_c_zones_carry_their_tags() {
    use Tag::*;
    let expectations: &[(&str, &[Tag], &[Tag])] = &[
        ("UTC", &[Fixed, WholeHour], &[Link, Legacy]),
        ("Etc/UTC", &[Fixed, WholeHour, Link], &[Legacy]),
        ("Etc/GMT", &[Fixed], &[Link]),
        ("GMT", &[Fixed, Link, Legacy], &[]),
        ("Zulu", &[Fixed, Link, Legacy], &[]),
        ("UCT", &[Fixed, Link, Legacy], &[]),
        (
            "Etc/GMT+5",
            &[Fixed, InvertedSign],
            &[Link, Legacy, Seconds],
        ),
        ("Etc/GMT-14", &[Fixed, InvertedSign], &[Link]),
        ("Factory", &[Fixed, Undefined, Legacy], &[]),
        ("Asia/Calcutta", &[Fixed, Fractional, Link], &[Legacy]),
        (
            "Asia/Kolkata",
            &[Fixed, Fractional, Seconds],
            &[Link, WholeHour],
        ),
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
        (
            "Europe/London",
            &[WholeHour, Dst, BaseChange],
            &[Midnight, SameOffset],
        ),
        ("Europe/Dublin", &[WholeHour, Dst], &[DstOther]),
        ("Africa/Casablanca", &[Multi, Midnight], &[Dst, SameOffset]),
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
        (
            "America/St_Johns",
            &[Fractional, Dst],
            &[Midnight, SameOffset],
        ),
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
        ("Asia/Riyadh", &[Fixed, WholeHour, Seconds], &[]),
        ("Asia/Manila", &[WholeHour, Seconds, WholeDay], &[Fixed]),
        ("Pacific/Guam", &[WholeHour, WholeDay], &[Fixed]),
        (
            "America/Metlakatla",
            &[Dst, Multi, BaseChange, WholeDay],
            &[],
        ),
        (
            "Antarctica/Macquarie",
            &[Dst, BaseChange],
            &[Midnight, SameOffset],
        ),
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
    assert!(!dublin.standard.is_dst);
    assert_eq!(dublin.dst.unwrap().kind.utoff, 0);
    assert!(dublin.dst.unwrap().kind.is_dst);
    for entry in all_names() {
        let footer = entry.zone.footer.unwrap();
        assert!(!footer.standard.is_dst, "{}", entry.name);
        if let Some(dst) = footer.dst {
            assert!(dst.kind.is_dst, "{}", entry.name);
        }
    }
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
    assert_eq!(
        lookup("Asia/Riyadh").unwrap().unwrap().zone.initial.utoff,
        3 * 3600 + 6 * 60 + 52
    );
    assert_eq!(
        lookup("Asia/Manila").unwrap().unwrap().zone.initial.utoff,
        -(15 * 3600 + 56 * 60 + 8)
    );
}

#[test]
fn seconds_tag_covers_the_whole_history() {
    for entry in all_names() {
        let zone = entry.zone;
        let footer = zone.footer.unwrap();
        let mut offsets = vec![zone.initial.utoff, footer.standard.utoff];
        offsets.extend(zone.transitions.iter().map(|t| t.kind.utoff));
        offsets.extend(footer.dst.map(|dst| dst.kind.utoff));
        assert_eq!(
            entry.tags.contains(Tag::Seconds),
            offsets.iter().any(|o| o % 60 != 0),
            "{}",
            entry.name
        );
    }
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

fn fnv1a64(data: &[u8]) -> String {
    let hash = data.iter().fold(0xcbf29ce484222325u64, |h, &byte| {
        (h ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

#[test]
fn table_is_reproducible() {
    assert_eq!(fnv1a64(TABLE_TEXT.as_bytes()), CHECKSUM);
    assert_eq!(fnv1a64(b""), "cbf29ce484222325");
    assert_eq!(fnv1a64(b"a"), "af63dc4c8601ec8c");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/generate_tzdata.py");
    let out = tempfile::tempdir().unwrap();
    let run = Command::new("uv")
        .args(["run", "--script"])
        .arg(&script)
        .arg("--out")
        .arg(out.path())
        .output();
    let output = match run {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        other => other.unwrap(),
    };
    assert!(
        output.status.success(),
        "generate_tzdata.py failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (name, expected) in [
        ("tzdata.txt", TABLE_TEXT),
        ("version.txt", VERSION),
        ("checksum.txt", CHECKSUM),
    ] {
        let written = std::fs::read_to_string(out.path().join(name)).unwrap();
        assert_eq!(written, expected, "{name}");
    }
    let compressed = std::fs::read(out.path().join("tzdata.txt.z")).unwrap();
    assert_eq!(compressed, COMPRESSED_TABLE);
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
    let footer = zone.footer.unwrap();
    assert!(!footer.standard.is_dst);
    let dst = footer.dst.unwrap();
    assert_eq!(
        dst.kind,
        LocalTimeType {
            utoff: 3600,
            is_dst: true,
            abbreviation: 1
        }
    );
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
        ("V x\nZ Etc/UTC 0 fixed\nA UTC\n", "has no initial record"),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nI 0 0 0\nI 0 0 0\n",
            "initial record repeated or out of order",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nT 0 0 0 0\nI 0 0 0\n",
            "initial record repeated or out of order",
        ),
        (
            "V x\nZ Etc/UTC 0 fixed\nA UTC\nF 0 0\nI 0 0 0\n",
            "initial record repeated or out of order",
        ),
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
