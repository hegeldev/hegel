//! TZif files (RFC 9636) as zone data, for zone sources
//! (`libhegel/docs/zone-sources.md` §2): a port of `parse_tzif` and
//! `parse_footer` in `scripts/generate_tzdata.py`, which read the version
//! 2+ 64-bit block and the POSIX TZ footer, the forms zic writes. A
//! malformed file is an error naming what is wrong, never a panic.
//!
//! The result is a [`Zone`] whose abbreviations are listed once each, in
//! the order the generator script lists them: the initial type's, then each
//! transition's, then the footer's standard and daylight ones. Type 0 is
//! the initial type, as RFC 9636 specifies.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{DstRule, Footer, LocalTimeType, RuleDate, RuleDay, Transition, Zone};

const HOUR: i32 = 3_600;

/// A local time type as the file states it, before abbreviations are
/// numbered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneType {
    pub utoff: i32,
    pub is_dst: bool,
    pub abbreviation: String,
}

/// A parsed footer before abbreviations are numbered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FooterRule {
    pub standard: ZoneType,
    pub dst: Option<(ZoneType, RuleDate, RuleDate)>,
}

/// Numbers abbreviations in first-seen order, each once.
#[derive(Default)]
pub struct Abbreviations {
    list: Vec<String>,
}

impl Abbreviations {
    /// `raw` with its abbreviation numbered.
    pub fn local_type(&mut self, raw: &ZoneType) -> LocalTimeType {
        let index = match self.list.iter().position(|a| *a == raw.abbreviation) {
            Some(index) => index,
            None => {
                self.list.push(raw.abbreviation.clone());
                self.list.len() - 1
            }
        };
        LocalTimeType {
            utoff: raw.utoff,
            is_dst: raw.is_dst,
            abbreviation: index,
        }
    }

    /// `raw` as a footer.
    pub fn footer(&mut self, raw: &FooterRule) -> Footer {
        Footer {
            standard: self.local_type(&raw.standard),
            dst: raw.dst.as_ref().map(|(kind, start, end)| DstRule {
                kind: self.local_type(kind),
                start: *start,
                end: *end,
            }),
        }
    }

    /// The numbered abbreviations.
    pub fn into_list(self) -> Vec<String> {
        self.list
    }
}

/// The zone `name` whose data are `initial`, `transitions` and `footer`,
/// with its abbreviations numbered in the generator script's order.
pub fn build_zone(
    name: &str,
    initial: &ZoneType,
    transitions: &[(i64, ZoneType)],
    footer: Option<&FooterRule>,
) -> Zone {
    let mut abbreviations = Abbreviations::default();
    let initial = abbreviations.local_type(initial);
    let transitions = transitions
        .iter()
        .map(|(instant, raw)| Transition {
            instant: *instant,
            kind: abbreviations.local_type(raw),
        })
        .collect();
    let footer = footer.map(|f| abbreviations.footer(f));
    Zone {
        name: name.to_string(),
        abbreviations: abbreviations.into_list(),
        initial,
        transitions,
        footer,
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: u64, what: &str) -> Result<&'a [u8], String> {
        let end = usize::try_from(len)
            .ok()
            .and_then(|len| self.pos.checked_add(len))
            .filter(|&end| end <= self.data.len())
            .ok_or_else(|| format!("the file ends inside its {what}"))?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn header(&mut self, which: &str) -> Result<[u64; 6], String> {
        let header = self.take(44, which)?;
        if &header[..4] != b"TZif" {
            return Err(format!("the {which} does not start with \"TZif\""));
        }
        let mut counts = [0u64; 6];
        for (i, count) in counts.iter_mut().enumerate() {
            let at = 20 + 4 * i;
            *count = u64::from(u32::from_be_bytes([
                header[at],
                header[at + 1],
                header[at + 2],
                header[at + 3],
            ]));
        }
        Ok(counts)
    }
}

/// Parse a version 2+ TZif file into the zone `name`; see the module docs.
pub fn parse_tzif(name: &str, data: &[u8]) -> Result<Zone, String> {
    let mut reader = Reader { data, pos: 0 };
    let [isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt] = reader.header("first header")?;
    let version = data[4];
    if version < b'2' {
        return Err(format!(
            "TZif version {version:#04x} has no 64-bit block; RFC 9636 version 2 or later is needed"
        ));
    }
    reader.take(
        timecnt * 5 + typecnt * 6 + charcnt + leapcnt * 8 + isstdcnt + isutcnt,
        "version 1 block",
    )?;
    let [isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt] = reader.header("second header")?;
    let times: Vec<i64> = reader
        .take(timecnt * 8, "transition times")?
        .chunks_exact(8)
        .map(|b| i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
        .collect();
    let indices = reader.take(timecnt, "transition types")?;
    let raw_types = reader.take(typecnt * 6, "local time types")?;
    let chars = reader.take(charcnt, "designations")?;
    reader.take(leapcnt * 12 + isstdcnt + isutcnt, "leap second records")?;
    let footer_bytes = &data[reader.pos..];
    let footer_text = match footer_bytes {
        [b'\n', text @ .., b'\n'] => text,
        _ => return Err("the footer is not newline-delimited".to_string()),
    };
    let footer_text = core::str::from_utf8(footer_text)
        .ok()
        .filter(|text| text.is_ascii())
        .ok_or("the footer is not ASCII")?;
    let types = raw_types
        .chunks_exact(6)
        .map(|b| {
            let utoff = i32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            let start = usize::from(b[5]);
            let designation = chars
                .get(start..)
                .and_then(|rest| rest.iter().position(|&c| c == 0).map(|end| &rest[..end]))
                .ok_or_else(|| {
                    format!("designation index {start} is not a NUL-terminated string")
                })?;
            let abbreviation = core::str::from_utf8(designation)
                .ok()
                .filter(|text| text.is_ascii())
                .ok_or("a designation is not ASCII")?;
            Ok(ZoneType {
                utoff,
                is_dst: b[4] != 0,
                abbreviation: abbreviation.to_string(),
            })
        })
        .collect::<Result<Vec<ZoneType>, String>>()?;
    let initial = types.first().ok_or("the file has no local time types")?;
    let transitions = times
        .iter()
        .zip(indices)
        .map(|(&instant, &index)| {
            types
                .get(usize::from(index))
                .map(|kind| (instant, kind.clone()))
                .ok_or_else(|| format!("transition type index {index} is out of range"))
        })
        .collect::<Result<Vec<(i64, ZoneType)>, String>>()?;
    if transitions.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err("transitions are not strictly ascending".to_string());
    }
    let footer = if footer_text.is_empty() {
        None
    } else {
        Some(parse_footer(footer_text)?)
    };
    Ok(build_zone(name, initial, &transitions, footer.as_ref()))
}

/// The length of the longest prefix of `text` whose bytes satisfy `pred`,
/// up to `max`.
fn run(text: &str, max: usize, pred: impl Fn(u8) -> bool) -> usize {
    text.bytes().take(max).take_while(|&b| pred(b)).count()
}

/// The `[+-]?hh[:mm[:ss]]` prefix of `text` as signed seconds and its
/// length, or `None` when `text` does not start with one.
fn posix_time_prefix(text: &str) -> Option<(i32, usize)> {
    let sign = usize::from(text.starts_with(['+', '-']));
    let hours = run(&text[sign..], 3, |b| b.is_ascii_digit());
    if hours == 0 {
        return None;
    }
    let mut end = sign + hours;
    let mut total: i32 = text[sign..end].parse::<i32>().ok()? * HOUR;
    for unit in [60, 1] {
        let rest = &text[end..];
        if !rest.starts_with(':') {
            break;
        }
        let digits = run(&rest[1..], 2, |b| b.is_ascii_digit());
        if digits == 0 {
            break;
        }
        total += rest[1..1 + digits].parse::<i32>().ok()? * unit;
        end += 1 + digits;
    }
    Some((if text.starts_with('-') { -total } else { total }, end))
}

fn parse_posix_time(text: &str) -> Result<i32, String> {
    match posix_time_prefix(text) {
        Some((seconds, len)) if len == text.len() => Ok(seconds),
        _ => Err(format!("bad POSIX time {text:?}")),
    }
}

fn parse_posix_name(text: &str) -> Result<(String, &str), String> {
    if let Some(quoted) = text.strip_prefix('<') {
        if let Some(close) = quoted.find('>').filter(|&close| close > 0) {
            return Ok((quoted[..close].to_string(), &quoted[close + 1..]));
        }
    } else {
        let len = text.bytes().take_while(u8::is_ascii_alphabetic).count();
        if len >= 3 {
            return Ok((text[..len].to_string(), &text[len..]));
        }
    }
    Err(format!("bad POSIX name in {text:?}"))
}

fn parse_posix_offset(text: &str) -> Result<(i32, &str), String> {
    let (seconds, len) =
        posix_time_prefix(text).ok_or_else(|| format!("bad POSIX offset in {text:?}"))?;
    Ok((-seconds, &text[len..]))
}

fn parse_posix_rule(text: &str) -> Result<RuleDate, String> {
    let bad = || format!("bad POSIX rule {text:?}");
    let digits = |s: &str, max: usize| run(s, max, |b| b.is_ascii_digit());
    let (day, rest) = if let Some(julian) = text.strip_prefix('J') {
        let len = digits(julian, 3);
        let n: u16 = julian[..len].parse().map_err(|_| bad())?;
        if !(1..=365).contains(&n) {
            return Err(format!("bad Julian day in {text:?}"));
        }
        (RuleDay::Julian(n), &julian[len..])
    } else if let Some(month_rule) = text.strip_prefix('M') {
        let month_len = digits(month_rule, 2);
        let fields = month_rule.as_bytes();
        let shape = month_len > 0
            && fields.len() >= month_len + 4
            && fields[month_len] == b'.'
            && fields[month_len + 1].is_ascii_digit()
            && fields[month_len + 2] == b'.'
            && fields[month_len + 3].is_ascii_digit();
        if !shape {
            return Err(bad());
        }
        let month: u8 = month_rule[..month_len].parse().map_err(|_| bad())?;
        let week = fields[month_len + 1] - b'0';
        let weekday = fields[month_len + 3] - b'0';
        if !((1..=12).contains(&month) && (1..=5).contains(&week) && weekday <= 6) {
            return Err(format!("bad month rule {text:?}"));
        }
        (
            RuleDay::MonthWeekDay {
                month,
                week,
                weekday,
            },
            &month_rule[month_len + 4..],
        )
    } else {
        let len = digits(text, 3);
        let n: u16 = text[..len].parse().map_err(|_| bad())?;
        if n > 365 {
            return Err(format!("bad zero-based day in {text:?}"));
        }
        (RuleDay::ZeroBased(n), &text[len..])
    };
    let time = match rest.strip_prefix('/') {
        None if rest.is_empty() => 2 * HOUR,
        None => return Err(bad()),
        Some(time) => parse_posix_time(time)?,
    };
    if !(-167 * HOUR..=167 * HOUR).contains(&time) {
        return Err(format!("rule time out of range in {text:?}"));
    }
    Ok(RuleDate { day, time })
}

/// Parse a POSIX TZ string of the forms zic writes (RFC 9636 §3.3.1).
pub fn parse_footer(text: &str) -> Result<FooterRule, String> {
    let (std_name, rest) = parse_posix_name(text)?;
    let (std_utoff, rest) = parse_posix_offset(rest)?;
    let standard = ZoneType {
        utoff: std_utoff,
        is_dst: false,
        abbreviation: std_name,
    };
    if rest.is_empty() {
        return Ok(FooterRule {
            standard,
            dst: None,
        });
    }
    let (dst_name, rest) = parse_posix_name(rest)?;
    let (dst_utoff, rest) = if rest.starts_with(',') {
        (std_utoff + HOUR, rest)
    } else {
        parse_posix_offset(rest)?
    };
    let rules = rest
        .strip_prefix(',')
        .ok_or_else(|| format!("DST without a rule in {text:?}"))?;
    let (start, end) = match rules.split(',').collect::<Vec<&str>>().as_slice() {
        [start, end] => (parse_posix_rule(start)?, parse_posix_rule(end)?),
        _ => return Err(format!("DST needs exactly two rules in {text:?}")),
    };
    Ok(FooterRule {
        standard,
        dst: Some((
            ZoneType {
                utoff: dst_utoff,
                is_dst: true,
                abbreviation: dst_name,
            },
            start,
            end,
        )),
    })
}

#[cfg(test)]
#[path = "../../tests/embedded/tzdata/tzif_tests.rs"]
mod tests;
