//! Tests for `src/tzdata/tzif.rs`: the TZif and POSIX TZ parsers on real
//! files and on hand-built malformed ones.

use super::*;
use std::string::ToString;
use std::vec;

fn real(name: &str) -> &'static [u8] {
    jiff_tzdb::get(name).unwrap().1
}

#[test]
fn real_files_parse() {
    let zone = parse_tzif("America/New_York", real("America/New_York")).unwrap();
    assert_eq!(zone.name, "America/New_York");
    let footer = zone.footer.unwrap();
    assert_eq!(footer.standard.utoff, -18000);
    let dst = footer.dst.unwrap();
    assert_eq!(dst.kind.utoff, -14400);
    assert_eq!(
        dst.start,
        RuleDate {
            day: RuleDay::MonthWeekDay {
                month: 3,
                week: 2,
                weekday: 0
            },
            time: 7200
        }
    );
    assert_eq!(zone.abbreviations[zone.initial.abbreviation], "LMT");
    let tokyo = parse_tzif("Asia/Tokyo", real("Asia/Tokyo")).unwrap();
    assert!(tokyo.footer.unwrap().dst.is_none());
}

/// A TZif file: an empty version 1 block, then a version 2 block with
/// `types` (offset, flag, abbreviation), `transitions` (instant, type
/// index) and the footer text.
fn file(types: &[(i32, u8, &str)], transitions: &[(i64, u8)], footer: &str) -> Vec<u8> {
    let mut chars = Vec::new();
    let mut raw_types = Vec::new();
    for (utoff, isdst, abbr) in types {
        raw_types.extend(utoff.to_be_bytes());
        raw_types.push(*isdst);
        raw_types.push(chars.len() as u8);
        chars.extend(abbr.bytes());
        chars.push(0);
    }
    let header = |timecnt: usize, typecnt: usize, charcnt: usize| {
        let mut h = b"TZif2".to_vec();
        h.extend([0u8; 15]);
        for count in [0, 0, 0, timecnt, typecnt, charcnt] {
            h.extend((count as u32).to_be_bytes());
        }
        h
    };
    let mut out = header(0, 0, 0);
    out.extend(header(transitions.len(), types.len(), chars.len()));
    for (instant, _) in transitions {
        out.extend(instant.to_be_bytes());
    }
    out.extend(transitions.iter().map(|t| t.1));
    out.extend(raw_types);
    out.extend(chars);
    out.push(b'\n');
    out.extend(footer.bytes());
    out.push(b'\n');
    out
}

#[test]
fn a_built_file_round_trips() {
    let data = file(
        &[(3600, 0, "AAA"), (7200, 1, "BBB"), (3600, 0, "AAA")],
        &[(100, 1), (200, 2)],
        "AAA-1",
    );
    let zone = parse_tzif("Test/Zone", &data).unwrap();
    assert_eq!(
        zone.abbreviations,
        vec!["AAA".to_string(), "BBB".to_string()]
    );
    assert_eq!(zone.transitions.len(), 2);
    assert_eq!(zone.transitions[0].kind.utoff, 7200);
    assert!(zone.transitions[0].kind.is_dst);
    assert_eq!(zone.footer.unwrap().standard.utoff, 3600);
    let no_footer = parse_tzif("Test/Zone", &file(&[(0, 0, "Z")], &[], "")).unwrap();
    assert!(no_footer.footer.is_none());
}

fn error(data: &[u8]) -> String {
    parse_tzif("Test/Zone", data).unwrap_err()
}

#[test]
fn malformed_files_are_errors() {
    assert!(error(b"").contains("ends inside"));
    let mut bad_magic = file(&[(0, 0, "Z")], &[], "");
    bad_magic[0] = b'X';
    assert!(error(&bad_magic).contains("does not start"));
    let mut v1 = file(&[(0, 0, "Z")], &[], "");
    v1[4] = 0;
    assert!(error(&v1).contains("no 64-bit block"));
    let mut short_v1 = file(&[(0, 0, "Z")], &[], "");
    short_v1[39] = 200;
    assert!(error(&short_v1).contains("version 1 block"));
    let good = file(&[(0, 0, "Z")], &[(5, 0)], "");
    assert!(error(&good[..good.len() - 8]).contains("ends inside"));
    let mut second = good.clone();
    second[44] = b'X';
    assert!(error(&second).contains("second header"));
    assert!(error(&good[..good.len() - 1]).contains("newline"));
    let mut not_ascii = good.clone();
    let at = not_ascii.len() - 1;
    not_ascii.insert(at, 0xff);
    assert!(error(&not_ascii).contains("not ASCII"));
    assert!(error(&file(&[], &[], "")).contains("no local time types"));
    assert!(error(&file(&[(0, 0, "Z")], &[(5, 3)], "")).contains("out of range"));
    assert!(error(&file(&[(0, 0, "Z")], &[(5, 0), (5, 0)], "")).contains("ascending"));
    assert!(error(&file(&[(0, 0, "Z")], &[], "ZZZ")).contains("bad POSIX offset"));
    let mut desig = file(&[(0, 0, "Z")], &[], "");
    let desig_at = desig.len() - 2 - 2 - 1;
    desig[desig_at] = 9;
    assert!(error(&desig).contains("designation index"));
    let mut abbr = file(&[(0, 0, "Z")], &[], "");
    let abbr_at = abbr.len() - 2 - 2;
    abbr[abbr_at] = 0xc3;
    assert!(error(&abbr).contains("designation is not ASCII"));
}

fn footer(text: &str) -> Result<FooterRule, String> {
    parse_footer(text)
}

#[test]
fn posix_footers_parse_in_every_form() {
    let f = footer("<+0530>-5:30").unwrap();
    assert_eq!(f.standard.abbreviation, "+0530");
    assert_eq!(f.standard.utoff, 19800);
    assert!(f.dst.is_none());
    let f = footer("EST5EDT,M3.2.0,M11.1.0").unwrap();
    let (dst, start, end) = f.dst.unwrap();
    assert_eq!(dst.utoff, -14400);
    assert_eq!(start.time, 7200);
    assert_eq!(end.time, 7200);
    let f = footer("<-03>3<-02>2,M3.5.0/-2,M10.5.0/-1").unwrap();
    let (dst, start, end) = f.dst.unwrap();
    assert_eq!(dst.utoff, -7200);
    assert_eq!(start.time, -7200);
    assert_eq!(end.time, -3600);
    let f = footer("XXX0YYY,J60/1:30:15,300/+2").unwrap();
    let (_, start, end) = f.dst.unwrap();
    assert_eq!(start.day, RuleDay::Julian(60));
    assert_eq!(start.time, 5415);
    assert_eq!(end.day, RuleDay::ZeroBased(300));
    let f = footer("AAA+1:2BBB2").unwrap_err();
    assert!(f.contains("DST without a rule"), "{f}");
}

#[test]
fn malformed_footers_are_errors() {
    for (text, message) in [
        ("A0", "bad POSIX name"),
        ("<>0", "bad POSIX name"),
        ("AAA", "bad POSIX offset"),
        ("AAA0BBB,M3.2.0", "exactly two rules"),
        ("AAA0BBB,M3.2.0,M11.1.0,J1", "exactly two rules"),
        ("AAA0BBB,J0,J1", "bad Julian day"),
        ("AAA0BBB,J,J1", "bad POSIX rule"),
        ("AAA0BBB,J1234,J1", "bad POSIX rule"),
        ("AAA0BBB,M13.1.0,J1", "bad month rule"),
        ("AAA0BBB,M3.6.0,J1", "bad month rule"),
        ("AAA0BBB,M3.1.7,J1", "bad month rule"),
        ("AAA0BBB,M3.1,J1", "bad POSIX rule"),
        ("AAA0BBB,Mx.1.1,J1", "bad POSIX rule"),
        ("AAA0BBB,366,J1", "bad zero-based day"),
        ("AAA0BBB,x,J1", "bad POSIX rule"),
        ("AAA0BBB,J1/,J2", "bad POSIX time"),
        ("AAA0BBB,J1/1:,J2", "bad POSIX time"),
        ("AAA0BBB,J1/168,J2", "out of range"),
    ] {
        let e = footer(text).unwrap_err();
        assert!(e.contains(message), "{text}: {e}");
    }
}
