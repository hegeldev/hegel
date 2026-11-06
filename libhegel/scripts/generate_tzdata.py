#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["tzdata==2026.5"]
# ///
"""Generate the engine's bundled time zone table from the `tzdata` package.

Run from `libhegel/` (`just libhegel::tzdata` does this), with uv, which fetches the
pinned `tzdata` release from PyPI:

    scripts/generate_tzdata.py [--out DIR] [--check]

`--check` writes nothing and instead fails when the text table, version
or checksum in the output directory differ from what the script would
write, or the deflated table there does not inflate to the text table
(`just check-tzdata`); the deflated bytes themselves are not compared,
as they depend on the zlib build. The engine's `table_is_reproducible`
test does the same through `uv` when it is installed and otherwise falls
back to `checksum.txt`.

The `tzdata` package ships the compiled TZif files `zoneinfo` reads. This
script parses the version 2+ 64-bit block and the footer of every file
named in its `zones` list (RFC 9636), classifies each zone, and writes two
files to `src/tzdata/` (or `--out DIR`):

  - `tzdata.txt`: the table, reviewable text, one record per line.
  - `tzdata.txt.z`: the same bytes deflated (zlib format, level 9), which
    the engine embeds with `include_bytes!` and inflates into a `Lazy` on
    first use. The compressed file is derived from the text one; a test
    checks they agree, so only the text file needs reviewing.
  - `version.txt`: the IANA release alone, with no newline, so that
    `hegel_tzdb_version` can answer without inflating the table.
  - `checksum.txt`: the 64-bit FNV-1a hash of `tzdata.txt`, as sixteen
    hex digits with no newline, which the engine's tests compare against
    the text table so a hand edit is caught even where `uv` is missing.

Table format (fields are space-separated; `#` lines are comments):

    V <iana version>                      e.g. `V 2026e`
    Z <name> <rank> <tags>                starts a zone; <tags> is a
                                          comma-separated list, or `-`
    A <abbr> ...                          the zone's abbreviations
    I <utoff> <isdst> <abbr index>        the type in force before the
                                          first transition
    T <instant> <utoff> <isdst> <abbr index>
                                          one explicit transition (unix
                                          seconds, seconds east of UT)
    F <abbr index> <utoff>                footer rule without DST
    F <abbr index> <utoff> <abbr index> <utoff> <start> <end>
                                          footer rule with DST; <start> and
                                          <end> are `M<m>.<w>.<d>/<t>`,
                                          `J<n>/<t>` or `<n>/<t>` with <t>
                                          the signed seconds after local
                                          midnight (RFC 9636 allows -167h
                                          to 167h)
    L <name> <primary> <rank> <tags>      a link: <name> resolves to the
                                          zone <primary>

Zone records (`Z` and the lines that follow) come first, sorted by name,
then the `L` records, sorted by name. Every zone has an `F` line in the
current data; the engine tolerates a missing one.

The `tzdata.zi` source makes `UTC` a link to `Etc/UTC` and keeps `Etc/GMT`
a separate zone; this script makes `UTC` the primary zone and `Etc/UTC`,
`Etc/GMT` and every name that resolved to either (`Etc/UCT`, `Zulu`,
`GMT`, `Greenwich`, ...) a link to `UTC`, as ECMA-402 does and as the
design's §4, §B.2 and Appendix C assume, so that `UTC` is a primary name
and the shrink target whether or not links are allowed. A link's TZif
must agree with its primary's in every offset and transition; only the
`GMT` abbreviation of the `Etc/GMT` family is lost by the folding.

A TZif record that repeats the type in force before it (the same offset,
`isdst` flag and abbreviation) changes nothing, and the table omits it:
zic writes such records as the point where the footer rule takes over
(Europe/London's GMT-to-GMT record at 1996-01-01), and the engine applies
the footer from the last record it is given, so a zone's last record is
kept when the footer rule, applied from the record before it, would not
reproduce it (America/Nuuk's 2023-10-28 record, which postpones the
footer's DST to 2024). A record that changes only the `isdst` flag or
only the abbreviation stays, since the engine reports both.

Tags are derived from the offsets, never from the `isdst` flags, so the
main and rearguard formats agree (the design's §5.3). A transition below
is an explicit one that changes the offset or the abbreviation, or one
the footer rule makes after the last explicit one. A record that changes
only the `isdst` flag is not a transition (Morocco's 2018 reclassification
of +01 as standard time exists only in the main data). "After 1970" means
at any instant from 1970-01-01T00:00:00Z on, including the footer's
transitions and offsets.

  fixed           no transition after 1970, which excludes a DST rule
  whole_hour      every offset after 1970 is a whole number of hours
  fractional      some offset after 1970 is a whole number of minutes but
                  not of hours (+05:30, +05:45, -03:30)
  seconds         some offset in the zone's history, including before
                  1970, is not a whole number of minutes (local mean
                  time, Monrovia's -00:44:30 until 1972)
  dst             the footer has a DST rule whose offset differs from the
                  standard one by exactly one hour
  dst_other       the footer has a DST rule with a different size (Lord
                  Howe's 30 minutes, Troll's two hours)
  multi           some calendar year has more than two offset changes,
                  dating each change by the wall time it happens at (so
                  Africa/Tripoli's change at 1982-01-01 00:00 local counts
                  in 1982 although it is 1981-12-31T22:00Z)
  base_change     the lowest offset in force during some year after 1970
                  differs from the previous year's (Europe/Moscow 2011
                  and 2014, permanent changes generally)
  midnight        a transition after 1970 whose wall time before or after
                  it is 00:00 (Nuuk's footer `M3.5.0/-1` included)
  spans_midnight  a transition after 1970 whose wall times before and
                  after it fall on different dates
  whole_day       a transition anywhere in the zone's history that skips
                  or repeats a whole day (an offset change of at least 24
                  hours: Pacific/Apia 2011, Asia/Manila 1844)
  same_offset     a transition after 1970 that changes the abbreviation
                  but not the offset (Europe/Lisbon 1992, WEST to CET)
  undefined       the zone uses the `-00` undefined-offset abbreviation
                  (Factory, Antarctic stations before they opened)
  link            the name is a link to another zone
  legacy          the name is not `Area/Location` for a current area
                  (EST, EST5EDT, US/Eastern), excepting UTC
  inverted_sign   the `Etc/GMT+5` family, whose sign is POSIX's

The rank is the shrink ladder, in tiers: `UTC` first; then the simple
`Area/Location` names, those whose tags other than `seconds` and `link`
are exactly `whole_hour` with `fixed` or with `dst` (a whole-hour offset
that is fixed or has textbook one-hour DST, with no other irregularity
after 1970); then every other `Area/Location` name; then the `Etc/`
names by absolute offset; then the `legacy` names. Within a tier (and
within an offset for `Etc/`) names with fewer tags come first, so a zone
precedes its links, and ties break by name. Ranks are unique, so they
order every name.

Size: for 2026e the text table is 446,624 bytes and deflates to 80,710.
Measured on the release libhegel shared library (macOS arm64, 4,053,936
bytes without any table), embedding the deflated table adds 121,056 bytes
and embedding the text table adds 484,176, the inflater being linked
already for failure blobs; so the deflated file is what the engine
embeds.
"""

from __future__ import annotations

import argparse
import calendar
import re
import struct
import sys
import zlib
from dataclasses import dataclass, field
from datetime import date, datetime, time, timedelta, timezone
from pathlib import Path

import tzdata

TZDATA_PYPI_VERSION = "2026.5"
IANA_VERSION = "2026e"

LIBHEGEL = Path(__file__).resolve().parent.parent
DEFAULT_OUT_DIR = LIBHEGEL / "src" / "tzdata"
TABLE_NAME = "tzdata.txt"
COMPRESSED_NAME = "tzdata.txt.z"
VERSION_NAME = "version.txt"
CHECKSUM_NAME = "checksum.txt"

UNIX_1970 = 0
SECONDS_PER_DAY = 86400
SECONDS_PER_HOUR = 3600

CURRENT_AREAS = {
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
}

INVERTED_SIGN_RE = re.compile(r"^Etc/GMT[+-][1-9][0-9]*$")

TAGS = [
    "fixed",
    "whole_hour",
    "fractional",
    "seconds",
    "dst",
    "dst_other",
    "multi",
    "base_change",
    "midnight",
    "spans_midnight",
    "whole_day",
    "same_offset",
    "undefined",
    "link",
    "legacy",
    "inverted_sign",
]


@dataclass(frozen=True)
class LocalType:
    utoff: int
    isdst: bool
    abbr: str

    def is_transition_from(self, other: LocalType) -> bool:
        return (self.utoff, self.abbr) != (other.utoff, other.abbr)


@dataclass
class Transition:
    instant: int
    type: LocalType


@dataclass
class Change:
    """A transition in the sense of the module docstring: from `instant`
    on, `after` applies in place of `before`."""

    instant: int
    before: LocalType
    after: LocalType

    def changes_offset(self) -> bool:
        return self.before.utoff != self.after.utoff


@dataclass
class RuleDate:
    text: str
    time: int


@dataclass
class Footer:
    std: LocalType
    dst: LocalType | None = None
    start: RuleDate | None = None
    end: RuleDate | None = None


@dataclass
class Zone:
    name: str
    initial: LocalType
    transitions: list[Transition]
    footer: Footer | None
    tags: set[str] = field(default_factory=set)
    rank: int = -1


@dataclass
class Link:
    name: str
    primary: str
    tags: set[str] = field(default_factory=set)
    rank: int = -1


def tzdata_dir() -> Path:
    return Path(tzdata.__file__).resolve().parent


def read_zone_names() -> list[str]:
    return (tzdata_dir() / "zones").read_text().split()


def read_links() -> dict[str, str]:
    """`{link name: target name}` from the compact source `tzdata.zi`, with
    `UTC` made primary (see the module docstring)."""
    links: dict[str, str] = {}
    for line in (tzdata_dir() / "zoneinfo" / "tzdata.zi").read_text().splitlines():
        if line.startswith("L "):
            _, target, name = line.split()
            links[name] = target
    folded = {links.pop("UTC"), "Etc/GMT"}
    links = {name: "UTC" if target in folded else target for name, target in links.items()}
    links.update((name, "UTC") for name in folded)
    return links


def parse_tzif(data: bytes) -> tuple[LocalType, list[Transition], Footer | None]:
    """Parse the 64-bit block and footer of a version 2+ TZif file."""
    if data[:4] != b"TZif":
        raise ValueError("not a TZif file")
    version = data[4]
    if version < ord("2"):
        raise ValueError(f"TZif version {version!r} has no 64-bit block")

    def header(offset: int) -> tuple[int, int, int, int, int, int]:
        return struct.unpack(">6I", data[offset + 20 : offset + 44])

    isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt = header(0)
    offset = 44 + timecnt * 5 + typecnt * 6 + charcnt + leapcnt * 8 + isstdcnt + isutcnt
    if data[offset : offset + 4] != b"TZif":
        raise ValueError("second header missing")
    isutcnt, isstdcnt, leapcnt, timecnt, typecnt, charcnt = header(offset)
    offset += 44
    times = struct.unpack(f">{timecnt}q", data[offset : offset + 8 * timecnt])
    offset += 8 * timecnt
    type_indices = data[offset : offset + timecnt]
    offset += timecnt
    raw_types = [
        struct.unpack(">iBB", data[offset + 6 * i : offset + 6 * i + 6]) for i in range(typecnt)
    ]
    offset += 6 * typecnt
    chars = data[offset : offset + charcnt]
    offset += charcnt
    offset += leapcnt * 12 + isstdcnt + isutcnt
    footer_bytes = data[offset:]
    if len(footer_bytes) < 2 or footer_bytes[0] != 0x0A or footer_bytes[-1] != 0x0A:
        raise ValueError("footer is not newline-delimited")
    footer_text = footer_bytes[1:-1].decode("ascii")

    types = []
    for utoff, isdst, desig in raw_types:
        end = chars.index(b"\0", desig)
        types.append(LocalType(utoff, bool(isdst), chars[desig:end].decode("ascii")))
    if not types:
        raise ValueError("no local time types")
    transitions = [Transition(t, types[i]) for t, i in zip(times, type_indices)]
    if any(a.instant >= b.instant for a, b in zip(transitions, transitions[1:])):
        raise ValueError("transitions are not strictly ascending")
    footer = parse_footer(footer_text) if footer_text else None
    return types[0], transitions, footer


POSIX_NAME_RE = re.compile(r"(<[^>]+>|[A-Za-z]{3,})")
POSIX_OFFSET_RE = re.compile(r"([+-]?)(\d{1,3})(?::(\d{1,2})(?::(\d{1,2}))?)?")
POSIX_RULE_RE = re.compile(r"(J\d{1,3}|M\d{1,2}\.\d\.\d|\d{1,3})(?:/(.*))?")


def parse_posix_time(text: str) -> int:
    m = POSIX_OFFSET_RE.fullmatch(text)
    if m is None:
        raise ValueError(f"bad POSIX time {text!r}")
    sign, hours, minutes, seconds = m.groups()
    total = int(hours) * SECONDS_PER_HOUR + int(minutes or 0) * 60 + int(seconds or 0)
    return -total if sign == "-" else total


def parse_posix_name(text: str) -> tuple[str, str]:
    m = POSIX_NAME_RE.match(text)
    if m is None:
        raise ValueError(f"bad POSIX name in {text!r}")
    name = m.group(1)
    if name.startswith("<"):
        name = name[1:-1]
    return name, text[m.end() :]


def parse_posix_offset(text: str) -> tuple[int, str]:
    m = POSIX_OFFSET_RE.match(text)
    if m is None or not m.group(2):
        raise ValueError(f"bad POSIX offset in {text!r}")
    return -parse_posix_time(m.group(0)), text[m.end() :]


def parse_posix_rule(text: str) -> RuleDate:
    m = POSIX_RULE_RE.fullmatch(text)
    if m is None:
        raise ValueError(f"bad POSIX rule {text!r}")
    date, time = m.groups()
    if date.startswith("J"):
        if not 1 <= int(date[1:]) <= 365:
            raise ValueError(f"bad Julian day in {text!r}")
    elif date.startswith("M"):
        month, week, day = (int(x) for x in date[1:].split("."))
        if not (1 <= month <= 12 and 1 <= week <= 5 and 0 <= day <= 6):
            raise ValueError(f"bad month rule {text!r}")
    elif not 0 <= int(date) <= 365:
        raise ValueError(f"bad zero-based day in {text!r}")
    seconds = 2 * SECONDS_PER_HOUR if time is None else parse_posix_time(time)
    if not -167 * SECONDS_PER_HOUR <= seconds <= 167 * SECONDS_PER_HOUR:
        raise ValueError(f"rule time out of range in {text!r}")
    return RuleDate(date, seconds)


def parse_footer(text: str) -> Footer:
    """Parse a POSIX TZ string of the forms zic writes (RFC 9636 §3.3.1)."""
    std_name, rest = parse_posix_name(text)
    std_utoff, rest = parse_posix_offset(rest)
    footer = Footer(LocalType(std_utoff, False, std_name))
    if not rest:
        return footer
    dst_name, rest = parse_posix_name(rest)
    if rest.startswith(","):
        dst_utoff = std_utoff + SECONDS_PER_HOUR
    else:
        dst_utoff, rest = parse_posix_offset(rest)
    footer.dst = LocalType(dst_utoff, True, dst_name)
    if not rest.startswith(","):
        raise ValueError(f"DST without a rule in {text!r}")
    start, end = rest[1:].split(",")
    footer.start = parse_posix_rule(start)
    footer.end = parse_posix_rule(end)
    return footer


def utc_year(instant: int) -> int:
    return datetime.fromtimestamp(instant, tz=timezone.utc).year


def wall_year(change: Change) -> int:
    return utc_year(change.instant + change.before.utoff)


def year_start(year: int) -> int:
    return int(datetime(year, 1, 1, tzinfo=timezone.utc).timestamp())


def last_explicit_year(zone: Zone) -> int:
    return utc_year(zone.transitions[-1].instant) if zone.transitions else 1969


def zone_changes(zone: Zone) -> list[Change]:
    """The zone's transitions in instant order: the explicit ones that
    change the offset or abbreviation, then the footer rule's from the year of
    the last explicit transition through the second year after it (or
    through 1972 when the explicit ones end before 1970), so that every
    year the tags look at is covered."""
    changes: list[Change] = []
    previous = zone.initial
    for t in zone.transitions:
        if t.type.is_transition_from(previous):
            changes.append(Change(t.instant, previous, t.type))
        previous = t.type
    if zone.footer is not None:
        last = zone.transitions[-1].instant if zone.transitions else None
        last_year = last_explicit_year(zone)
        for year in range(last_year, max(last_year, 1970) + 3):
            for change in footer_transitions(zone.footer, year):
                if last is None or change.instant > last:
                    changes.append(change)
    changes.sort(key=lambda c: c.instant)
    return changes


def types_between(zone: Zone, changes: list[Change], start: int, end: int) -> set[LocalType]:
    """The local time types in force at some instant in `start <= t < end`."""
    current = zone.initial
    types: set[LocalType] = set()
    for change in changes:
        if change.instant >= end:
            break
        if change.instant <= start:
            current = change.after
        else:
            types.add(change.after)
    types.add(current)
    return types


def offsets_in_year(zone: Zone, changes: list[Change], year: int) -> set[int]:
    return {t.utoff for t in types_between(zone, changes, year_start(year), year_start(year + 1))}


def all_offsets(zone: Zone) -> set[int]:
    offsets = {zone.initial.utoff}
    offsets.update(t.type.utoff for t in zone.transitions)
    if zone.footer is not None:
        offsets.add(zone.footer.std.utoff)
        if zone.footer.dst is not None:
            offsets.add(zone.footer.dst.utoff)
    return offsets


def days_in_month(year: int, month: int) -> int:
    if month == 12:
        return 31
    return (date(year, month + 1, 1) - date(year, month, 1)).days


def rule_date(rule: RuleDate, year: int) -> date:
    """The calendar date a footer rule names in `year` (RFC 9636 §3.3.1)."""
    text = rule.text
    if text.startswith("J"):
        day = int(text[1:])
        if day >= 60 and calendar.isleap(year):
            day += 1
        return date(year, 1, 1) + timedelta(days=day - 1)
    if text.startswith("M"):
        month, week, weekday = (int(x) for x in text[1:].split("."))
        first = date(year, month, 1)
        day = 1 + (weekday - (first.weekday() + 1) % 7) % 7 + (week - 1) * 7
        if day > days_in_month(year, month):
            day -= 7
        return date(year, month, day)
    return date(year, 1, 1) + timedelta(days=int(text))


def footer_transitions(footer: Footer, year: int) -> list[Change]:
    """The footer rule's transitions in `year`."""
    if footer.dst is None:
        return []
    result = []
    for rule, before, after in (
        (footer.start, footer.std, footer.dst),
        (footer.end, footer.dst, footer.std),
    ):
        midnight = int(datetime.combine(rule_date(rule, year), time(), timezone.utc).timestamp())
        result.append(Change(midnight + rule.time - before.utoff, before, after))
    return result


def rule_type_at(footer: Footer, instant: int) -> LocalType | None:
    """The type the footer rule has in force at `instant`, or `None` when
    no rule transition precedes it within the years looked at."""
    year = utc_year(instant)
    changes = sorted(
        (c for y in range(year - 2, year + 1) for c in footer_transitions(footer, y)),
        key=lambda c: c.instant,
    )
    preceding = [c for c in changes if c.instant <= instant]
    return preceding[-1].after if preceding else None


def is_redundant(
    zone: Zone, record: Transition, preceding: Transition | None, last: bool
) -> bool:
    """Whether `record` repeats the type in force before it and the engine
    would answer every query the same without it (the module docstring).
    `preceding` is the kept record before it, if any, and `last` whether
    it is the zone's last record, where the footer rule takes over."""
    previous = zone.initial if preceding is None else preceding.type
    if record.type != previous:
        return False
    if not last:
        return True
    footer = zone.footer
    if footer is None:
        return True
    if footer.dst is None:
        return footer.std == record.type
    if preceding is None:
        return False
    start = preceding.instant
    years = range(utc_year(start), utc_year(record.instant) + 1)
    rule = [c for y in years for c in footer_transitions(footer, y)]
    if any(start < c.instant <= record.instant for c in rule):
        return False
    return rule_type_at(footer, start) == record.type


def prune_transitions(zone: Zone) -> None:
    """Drop the records that change nothing (the module docstring)."""
    kept: list[Transition] = []
    count = len(zone.transitions)
    for index, record in enumerate(zone.transitions):
        preceding = kept[-1] if kept else None
        if not is_redundant(zone, record, preceding, index + 1 == count):
            kept.append(record)
    zone.transitions = kept


def check_transitions(zone: Zone) -> None:
    """Fail unless every record changes the type in force, except for a
    last record that pins where the footer rule starts."""
    count = len(zone.transitions)
    for index, record in enumerate(zone.transitions):
        preceding = zone.transitions[index - 1] if index else None
        previous = zone.initial if preceding is None else preceding.type
        last = index + 1 == count
        if record.type == previous and (not last or is_redundant(zone, record, preceding, last)):
            raise SystemExit(f"{zone.name}: the record at {record.instant} changes nothing")


def compute_zone_tags(zone: Zone) -> set[str]:
    tags: set[str] = set()
    footer = zone.footer
    changes = zone_changes(zone)
    recent = [c for c in changes if c.instant >= UNIX_1970]
    recent_types = types_between(zone, changes, UNIX_1970, sys.maxsize)
    recent_offsets = {t.utoff for t in recent_types}

    if not recent:
        tags.add("fixed")
    if all(o % SECONDS_PER_HOUR == 0 for o in recent_offsets):
        tags.add("whole_hour")
    if any(o % SECONDS_PER_HOUR != 0 and o % 60 == 0 for o in recent_offsets):
        tags.add("fractional")
    if any(o % 60 != 0 for o in all_offsets(zone)):
        tags.add("seconds")
    if footer is not None and footer.dst is not None:
        delta = abs(footer.dst.utoff - footer.std.utoff)
        tags.add("dst" if delta == SECONDS_PER_HOUR else "dst_other")

    if any(abs(c.after.utoff - c.before.utoff) >= SECONDS_PER_DAY for c in changes):
        tags.add("whole_day")
    changes_per_year: dict[int, int] = {}
    for change in recent:
        if change.changes_offset():
            year = wall_year(change)
            changes_per_year[year] = changes_per_year.get(year, 0) + 1
        else:
            tags.add("same_offset")
        wall_before = change.instant + change.before.utoff
        wall_after = change.instant + change.after.utoff
        if wall_before % SECONDS_PER_DAY == 0 or wall_after % SECONDS_PER_DAY == 0:
            tags.add("midnight")
        if wall_before // SECONDS_PER_DAY != wall_after // SECONDS_PER_DAY:
            tags.add("spans_midnight")
    if any(n > 2 for n in changes_per_year.values()):
        tags.add("multi")

    previous_min = min(offsets_in_year(zone, changes, 1969))
    for year in range(1970, last_explicit_year(zone) + 2):
        current_min = min(offsets_in_year(zone, changes, year))
        if current_min != previous_min:
            tags.add("base_change")
        previous_min = current_min

    if any(t.abbr == "-00" for t in recent_types):
        tags.add("undefined")

    tags |= name_tags(zone.name)
    return tags


def name_tags(name: str) -> set[str]:
    tags: set[str] = set()
    area = name.split("/", 1)[0] if "/" in name else None
    if name != "UTC" and area not in CURRENT_AREAS:
        tags.add("legacy")
    if INVERTED_SIGN_RE.match(name):
        tags.add("inverted_sign")
    return tags


SIMPLE_TAGS = {"whole_hour", "fixed", "dst", "seconds", "link"}


def is_simple(tags: set[str]) -> bool:
    """Whether a name belongs to the ladder's tier after `UTC`: a whole-hour
    offset that is fixed or has one-hour DST, and nothing else after 1970
    (the module docstring)."""
    return "whole_hour" in tags and ("fixed" in tags or "dst" in tags) and tags <= SIMPLE_TAGS


def assign_ranks(zones: dict[str, Zone], links: dict[str, Link]) -> None:
    entries: list[tuple[str, set[str], int]] = []
    for z in zones.values():
        entries.append((z.name, z.tags, abs(z.footer.std.utoff) if z.footer else 0))
    for link in links.values():
        target = zones[link.primary]
        entries.append((link.name, link.tags, abs(target.footer.std.utoff) if target.footer else 0))

    def key(entry: tuple[str, set[str], int]) -> tuple:
        name, tags, abs_offset = entry
        if name == "UTC":
            return (0, 0, 0, name)
        if "legacy" in tags:
            return (4, 0, len(tags), name)
        if name.startswith("Etc/"):
            return (3, abs_offset, len(tags), name)
        if is_simple(tags):
            return (1, 0, len(tags), name)
        return (2, 0, len(tags), name)

    for rank, (name, _, _) in enumerate(sorted(entries, key=key)):
        if name in zones:
            zones[name].rank = rank
        else:
            links[name].rank = rank


def offsets_only(zone: Zone) -> tuple:
    """The zone's data without its abbreviations, for comparing a link's
    TZif with its primary's."""
    footer = zone.footer
    return (
        zone.initial.utoff,
        [(t.instant, t.type.utoff) for t in zone.transitions],
        None
        if footer is None
        else (
            footer.std.utoff,
            None if footer.dst is None else (footer.dst.utoff, footer.start, footer.end),
        ),
    )


def abbreviation_list(zone: Zone) -> list[str]:
    abbrs: list[str] = []

    def add(abbr: str) -> None:
        if abbr not in abbrs:
            abbrs.append(abbr)

    add(zone.initial.abbr)
    for t in zone.transitions:
        add(t.type.abbr)
    if zone.footer is not None:
        add(zone.footer.std.abbr)
        if zone.footer.dst is not None:
            add(zone.footer.dst.abbr)
    return abbrs


def format_tags(tags: set[str]) -> str:
    ordered = [t for t in TAGS if t in tags]
    return ",".join(ordered) if ordered else "-"


def format_type(t: LocalType, abbrs: list[str]) -> str:
    return f"{t.utoff} {int(t.isdst)} {abbrs.index(t.abbr)}"


def format_zone(zone: Zone) -> list[str]:
    abbrs = abbreviation_list(zone)
    lines = [f"Z {zone.name} {zone.rank} {format_tags(zone.tags)}"]
    lines.append("A " + " ".join(abbrs))
    lines.append(f"I {format_type(zone.initial, abbrs)}")
    for t in zone.transitions:
        lines.append(f"T {t.instant} {format_type(t.type, abbrs)}")
    f = zone.footer
    if f is not None:
        std = f"{abbrs.index(f.std.abbr)} {f.std.utoff}"
        if f.dst is None:
            lines.append(f"F {std}")
        else:
            dst = f"{abbrs.index(f.dst.abbr)} {f.dst.utoff}"
            start = f"{f.start.text}/{f.start.time}"
            end = f"{f.end.text}/{f.end.time}"
            lines.append(f"F {std} {dst} {start} {end}")
    return lines


def load_zone(zoneinfo: Path, name: str) -> Zone:
    initial, transitions, footer = parse_tzif((zoneinfo / name).read_bytes())
    zone = Zone(name, initial, transitions, footer)
    prune_transitions(zone)
    check_transitions(zone)
    return zone


def build_table() -> str:
    if tzdata.__version__ != TZDATA_PYPI_VERSION or tzdata.IANA_VERSION != IANA_VERSION:
        raise SystemExit(
            f"tzdata {tzdata.__version__} ({tzdata.IANA_VERSION}) is installed; "
            f"this script pins {TZDATA_PYPI_VERSION} ({IANA_VERSION})"
        )
    names = read_zone_names()
    link_targets = read_links()
    zoneinfo = tzdata_dir() / "zoneinfo"

    zones: dict[str, Zone] = {}
    links: dict[str, Link] = {}
    for name in names:
        if name in link_targets:
            links[name] = Link(name, link_targets[name])
            continue
        zones[name] = load_zone(zoneinfo, name)

    for link in links.values():
        if link.primary not in zones:
            raise SystemExit(f"link {link.name} targets {link.primary}, which is not a zone")
        if offsets_only(load_zone(zoneinfo, link.name)) != offsets_only(zones[link.primary]):
            raise SystemExit(f"link {link.name} differs from {link.primary}")

    for zone in zones.values():
        zone.tags = compute_zone_tags(zone)
    for link in links.values():
        link.tags = (zones[link.primary].tags - {"legacy", "inverted_sign"}) | {"link"}
        link.tags |= name_tags(link.name)
    assign_ranks(zones, links)

    lines = [
        f"# IANA time zone database {IANA_VERSION}, from the tzdata {TZDATA_PYPI_VERSION} PyPI package",
        "# Generated by scripts/generate_tzdata.py (`just tzdata`); do not edit by hand",
        f"V {IANA_VERSION}",
    ]
    for name in sorted(zones):
        lines.extend(format_zone(zones[name]))
    for name in sorted(links):
        link = links[name]
        lines.append(f"L {link.name} {link.primary} {link.rank} {format_tags(link.tags)}")
    return "\n".join(lines) + "\n"


def fnv1a64(data: bytes) -> str:
    h = 0xCBF29CE484222325
    for byte in data:
        h = ((h ^ byte) * 0x100000001B3) % (1 << 64)
    return f"{h:016x}"


def inflate(path: Path) -> bytes | None:
    """The deflated table at `path`, or `None` when it is missing or does
    not inflate."""
    try:
        return zlib.decompress(path.read_bytes())
    except (OSError, zlib.error):
        return None


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT_DIR)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    table = build_table().encode("ascii")
    files = {
        TABLE_NAME: table,
        VERSION_NAME: IANA_VERSION.encode("ascii"),
        CHECKSUM_NAME: fnv1a64(table).encode("ascii"),
    }
    if args.check:
        stale = [
            name
            for name, content in files.items()
            if not (args.out / name).is_file() or (args.out / name).read_bytes() != content
        ]
        if inflate(args.out / COMPRESSED_NAME) != table:
            stale.append(COMPRESSED_NAME)
        if stale:
            print(f"out of date in {args.out}: {', '.join(stale)}; run `just tzdata`")
            return 1
        print(f"{args.out / TABLE_NAME} is up to date")
        return 0
    compressed = zlib.compress(table, 9)
    args.out.mkdir(parents=True, exist_ok=True)
    for name, content in files.items():
        (args.out / name).write_bytes(content)
    (args.out / COMPRESSED_NAME).write_bytes(compressed)
    print(f"wrote {args.out / TABLE_NAME} ({len(table)} bytes, {len(compressed)} deflated)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
