from __future__ import annotations

import datetime as dt
import gc
import os
import struct
import sys
import threading
import time
import weakref
import zoneinfo
from importlib.resources import files

import pytest

import hegel
from hegel import _native
from hegel import generators as gs
from hegel.generators.temporal import (
    RuleDate,
    ZoneData,
    ZoneNameInfo,
    ZoneOracle,
    ZoneRule,
    ZoneSource,
    ZoneType,
    _is_imaginary,
    _zoneinfo_tzif,
    _zoneinfo_version,
)
from tests.helpers import failing, find_example, run_test

UTC = dt.UTC
HOUR = dt.timedelta(hours=1)
GMT = ZoneType(dt.timedelta(0), False, "GMT")
BST = ZoneType(HOUR, True, "BST")


def tzif(types, transitions, footer):
    """A version 2 TZif file: ``types`` as ``(utoff, is_dst, abbreviation)``,
    ``transitions`` as ``(instant, type index)``, and the POSIX TZ string
    ``footer``."""
    chars = b""
    indices = []
    for _, _, abbreviation in types:
        indices.append(len(chars))
        chars += abbreviation.encode() + b"\0"

    def block(time_format):
        header = b"TZif2" + bytes(15)
        header += struct.pack(">6l", 0, 0, 0, len(transitions), len(types), len(chars))
        body = b"".join(struct.pack(time_format, instant) for instant, _ in transitions)
        body += bytes(index for _, index in transitions)
        body += b"".join(
            struct.pack(">lbB", utoff, is_dst, index)
            for (utoff, is_dst, _), index in zip(types, indices, strict=True)
        )
        return header + body + chars

    return block(">l") + block(">q") + b"\n" + footer.encode() + b"\n"


def timestamp(*fields):
    return int(dt.datetime(*fields, tzinfo=UTC).timestamp())


# A London whose clocks went forward at noon on 2020-06-15 and back at noon
# on 2020-09-01, and never otherwise.
FAKE_LONDON = tzif(
    [(0, False, "XMT"), (3600, True, "XST")],
    [(timestamp(2020, 6, 15, 12), 1), (timestamp(2020, 9, 1, 11), 0)],
    "XMT0",
)


@pytest.fixture
def tzpath(tmp_path):
    """Point zoneinfo at an empty directory, ``tmp_path / "zoneinfo"``, for
    the test, restoring its search path afterwards. Tests using it build
    zones with ``ZoneInfo.no_cache``, so the cache never holds a zone read
    from the directory."""
    directory = tmp_path / "zoneinfo"
    directory.mkdir()
    previous = zoneinfo.TZPATH
    zoneinfo.reset_tzpath([str(directory)])
    yield directory
    zoneinfo.reset_tzpath(previous)


@pytest.fixture
def default_source():
    previous = gs.temporal.default_zone_source()
    yield previous
    gs.temporal.set_default_zone_source(previous)


def kind_of(source, name, wall_time):
    return source.resolve(name, wall_time).kind


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


def agrees_with_zoneinfo(value, source):
    resolution = source.resolve(value.tzinfo.key, value.replace(tzinfo=None))
    return resolution.instant == value.astimezone(UTC)


def test_the_default_source_is_zoneinfo():
    source = gs.temporal.default_zone_source()
    assert repr(source) == "ZoneSource.zoneinfo()"
    assert source.version == _zoneinfo_version()
    check_all(
        gs.temporal.zoned_datetimes(dt.datetime(1990, 1, 1), dt.datetime(2030, 1, 1)),
        lambda value: agrees_with_zoneinfo(value, source),
        test_cases=300,
    )


def test_the_zoneinfo_source_follows_zoneinfo_search_path(tzpath, monkeypatch):
    (tzpath / "Europe").mkdir()
    (tzpath / "Europe" / "London").write_bytes(FAKE_LONDON)
    monkeypatch.setitem(sys.modules, "tzdata", None)
    source = ZoneSource.zoneinfo()
    london = zoneinfo.ZoneInfo.no_cache("Europe/London")
    assert london.utcoffset(dt.datetime(2020, 4, 1)) == dt.timedelta(0)
    assert kind_of(source, "Europe/London", dt.datetime(2020, 6, 15, 12, 30)) == "gap"
    assert kind_of(source, "Europe/London", dt.datetime(2020, 3, 29, 1, 30)) == "unique"
    real = ZoneSource.bundled()
    assert kind_of(real, "Europe/London", dt.datetime(2020, 6, 15, 12, 30)) == "unique"
    in_london = gs.temporal.zones(source=source, no_cache=True)
    june = gs.temporal.zoned_datetimes(
        dt.datetime(2020, 6, 15, 11, 30), dt.datetime(2020, 6, 15, 12, 30), zones=in_london
    )
    gap = find_example(june, _is_imaginary)
    assert (gap.date(), gap.hour) == (dt.date(2020, 6, 15), 12)
    assert gap.tzinfo.key == "Europe/London"
    check_all(
        gs.temporal.zoned_datetimes(
            dt.datetime(2020, 6, 15, 11), dt.datetime(2020, 6, 15, 14), zones=in_london
        ),
        lambda value: agrees_with_zoneinfo(value, source),
        test_cases=100,
    )
    september = gs.temporal.zoned_datetimes(
        dt.datetime(2020, 9, 1, 10, 30), dt.datetime(2020, 9, 1, 11, 30), zones=in_london
    )
    assert find_example(september, lambda value: value.fold == 1).hour == 11
    check_all(
        gs.temporal.zoned_datetimes(
            dt.datetime(2020, 6, 15),
            dt.datetime(2020, 6, 16),
            zones=in_london,
            allow_imaginary=False,
        ),
        lambda value: not _is_imaginary(value),
    )


def test_the_zoneinfo_source_names_and_version(tzpath, monkeypatch):
    (tzpath / "Europe").mkdir()
    (tzpath / "Europe" / "London").write_bytes(FAKE_LONDON)
    (tzpath / "Europe" / "Olympus_Mons").write_bytes(FAKE_LONDON)
    (tzpath / "zone.tab").write_text("not a zone\n")
    monkeypatch.setitem(sys.modules, "tzdata", None)
    source = ZoneSource.zoneinfo()
    assert source.version == ""
    names = gs.temporal.zone_names(source=source)
    check_all(names, lambda name: name in ("Europe/London", "Europe/Olympus_Mons"))
    assert find_example(names, lambda name: name != "Europe/London") == "Europe/Olympus_Mons"
    assert repr(names) == "temporal.zone_names(source=ZoneSource.zoneinfo())"
    with pytest.raises(hegel.InvalidArgument, match='unknown time zone "zone.tab"'):
        gs.temporal.zone_names(names=["zone.tab"], source=source)
    with pytest.raises(hegel.InvalidArgument, match='unknown time zone "Europe"'):
        gs.temporal.zone_names(names=["Europe"], source=source)
    assert _zoneinfo_tzif("../Europe/London") is None
    assert _zoneinfo_tzif("Europe/London") == FAKE_LONDON
    (tzpath / "tzdata.zi").write_text("# version 2099z\n")
    assert ZoneSource.zoneinfo().version == "2099z"
    (tzpath / "tzdata.zi").write_text("# not a version\n")
    assert ZoneSource.zoneinfo().version == ""
    (tzpath / "+VERSION").write_text("2098y\n")
    assert ZoneSource.zoneinfo().version == "2098y"


def test_the_zoneinfo_source_reads_the_tzdata_package(tmp_path):
    previous = zoneinfo.TZPATH
    zoneinfo.reset_tzpath([str(tmp_path / "missing")])
    try:
        import tzdata

        source = ZoneSource.zoneinfo()
        assert source.version == tzdata.IANA_VERSION
        london = (files("tzdata.zoneinfo") / "Europe" / "London").read_bytes()
        assert _zoneinfo_tzif("Europe/London") == london
        check_all(
            gs.temporal.zoned_datetimes(
                dt.datetime(1990, 1, 1),
                dt.datetime(2030, 1, 1),
                zones=gs.temporal.zones(source=source, no_cache=True),
            ),
            lambda value: agrees_with_zoneinfo(value, source),
            test_cases=200,
        )
    finally:
        zoneinfo.reset_tzpath(previous)


def zoneinfo_finds(name):
    from zoneinfo import _common, _tzpath

    try:
        path = _tzpath.find_tzfile(name)
        if path is None:
            with _common.load_tzdata(name) as file:
                data = file.read()
        else:
            with open(path, "rb") as file:
                data = file.read()
    except (ValueError, OSError, zoneinfo.ZoneInfoNotFoundError):
        return None
    return data if data[:4] == b"TZif" else None


@pytest.mark.parametrize(
    "name",
    [
        "Europe/London",
        "Europe/Olympus_Mons",
        "America/New_York",
        "UTC",
        "Europe",
        "zone.tab",
        "Nowhere/Nothing",
        "",
        "/etc/localtime",
        os.path.join(os.path.abspath(os.sep), "etc", "localtime"),
        "../Europe/London",
        "Europe/../UTC",
        "./UTC",
        "Europe//London",
        "Europe/London/",
        "\udcff",
    ],
)
def test_the_zoneinfo_lookup_finds_what_zoneinfo_finds(tzpath, name):
    (tzpath / "Europe").mkdir()
    (tzpath / "Europe" / "Olympus_Mons").write_bytes(FAKE_LONDON)
    (tzpath / "zone.tab").write_text("not a zone\n")
    assert _zoneinfo_tzif(name) == zoneinfo_finds(name)


def test_no_zoneinfo_database(tzpath, monkeypatch):
    monkeypatch.setitem(sys.modules, "tzdata", None)
    assert _zoneinfo_version() == ""
    zoneinfo.reset_tzpath([str(tzpath / "missing")])
    assert _zoneinfo_version() == ""
    source = ZoneSource.zoneinfo()
    with pytest.raises(hegel.InvalidArgument, match="install the tzdata package"):
        gs.temporal.zone_names(source=source)
    with pytest.raises(hegel.InvalidArgument, match="install the tzdata package"):
        gs.temporal.zoned_datetimes(source=source)
    # The bundled database needs nothing from the system.
    check_all(
        gs.temporal.zone_names(source=ZoneSource.bundled()),
        lambda name: name in ZoneSource.bundled(),
    )


def test_the_deprecated_timezones_draws_utc_without_a_zoneinfo_database(tzpath, monkeypatch):
    monkeypatch.setitem(sys.modules, "tzdata", None)
    with pytest.warns(DeprecationWarning, match="use gs.temporal.zones"):
        zones = gs.timezones()
    check_all(zones, lambda zone: zone is UTC)
    with pytest.warns(DeprecationWarning):
        check_all(gs.datetimes(timezones=gs.timezones()), lambda value: value.tzinfo is UTC)


def test_the_bundled_source():
    source = ZoneSource.bundled()
    assert repr(source) == "ZoneSource.bundled()"
    assert source.version == gs.temporal.tzdb_version()
    zones = gs.temporal.zones(source=source)
    assert repr(zones) == "temporal.zones(source=ZoneSource.bundled())"
    check_all(zones, lambda zone: isinstance(zone, zoneinfo.ZoneInfo))
    with pytest.raises(hegel.InvalidArgument):
        source.version = "2000a"
    with pytest.raises(hegel.InvalidArgument):
        source.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)


def test_setting_the_default_source(default_source):
    custom = ZoneSource()
    custom.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)
    assert gs.temporal.set_default_zone_source(custom) is default_source
    assert gs.temporal.default_zone_source() is custom
    check_all(gs.temporal.zone_names(), lambda name: name == "Europe/Olympus_Mons")
    assert repr(gs.temporal.zone_names()) == "temporal.zone_names()"
    assert gs.temporal.set_default_zone_source(default_source) is custom
    assert gs.temporal.default_zone_source() is default_source
    with pytest.raises(hegel.InvalidArgument, match="source=None must be a ZoneSource"):
        gs.temporal.set_default_zone_source(None)
    with pytest.raises(hegel.InvalidArgument, match="source='bundled' must be a ZoneSource"):
        gs.temporal.zone_names(source="bundled")
    with pytest.raises(hegel.InvalidArgument, match="source='bundled' must be a ZoneSource"):
        gs.temporal.zoned_datetimes(source="bundled")


# 12:00 to 12:59 on 2020-06-15 was skipped in FAKE_LONDON and is an ordinary
# hour in the real London, so only a draw against the fake data finds no
# real wall time in it.
IN_THE_FAKE_GAP = (dt.datetime(2020, 6, 15, 12, 10), dt.datetime(2020, 6, 15, 12, 50))


def rejects_every_test_case(generator):
    def body(tc):
        tc.draw(generator)

    failing(body, hegel.FailedHealthCheck, test_cases=5)


def test_a_generator_keeps_the_default_source_it_was_built_with(default_source):
    london = zoneinfo.ZoneInfo("Europe/London")
    custom = ZoneSource()
    custom.add_tzif("Europe/London", FAKE_LONDON)

    def build():
        return (
            gs.temporal.zone_names(),
            gs.temporal.zoned_datetimes(),
            gs.temporal.datetimes(*IN_THE_FAKE_GAP, zones=gs.just(london), allow_imaginary=False),
        )

    before = build()
    assert gs.temporal.set_default_zone_source(custom) is default_source
    after = build()
    names, zoned, in_london = before
    for _ in range(2):
        assert find_example(names, lambda name: name != "Europe/London") != "Europe/London"
        assert find_example(zoned, lambda value: value.tzinfo is not london).tzinfo is not london
        check_all(in_london, lambda value: value.tzinfo is london)
        gs.temporal.set_default_zone_source(default_source)
    names, zoned, in_london = after
    check_all(names, lambda name: name == "Europe/London")
    check_all(zoned, lambda value: value.tzinfo is london)
    rejects_every_test_case(in_london)


def test_a_generator_reads_its_sources_names_when_it_is_built():
    source = ZoneSource()
    source.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)
    before = gs.temporal.zone_names(source=source)
    source.add_tzif("Europe/Pavonis_Mons", FAKE_LONDON)
    after = gs.temporal.zone_names(source=source)
    check_all(before, lambda name: name == "Europe/Olympus_Mons")
    assert find_example(after, lambda name: name != "Europe/Olympus_Mons") == "Europe/Pavonis_Mons"
    with pytest.raises(hegel.InvalidArgument, match="no allowed name"):
        gs.temporal.zone_names(source=ZoneSource())


def test_zones_from_another_generator_are_drawn_against_the_given_source():
    london, utc = zoneinfo.ZoneInfo("Europe/London"), zoneinfo.ZoneInfo("UTC")
    custom = ZoneSource()
    custom.add_tzif("Europe/London", FAKE_LONDON)
    custom.add_tzif("UTC", (files("tzdata.zoneinfo") / "UTC").read_bytes())
    wrapped = [
        gs.temporal.zones(source=custom).filter(lambda zone: True),
        gs.temporal.zones(source=custom).map(lambda zone: zone),
        gs.one_of(gs.temporal.zones(source=custom), gs.just(utc)),
        gs.sampled_from([london, utc]),
    ]
    for zones in wrapped:
        for generator in (gs.temporal.datetimes, gs.temporal.zoned_datetimes):
            against_custom = generator(
                *IN_THE_FAKE_GAP, zones=zones, source=custom, allow_imaginary=False
            )
            # London has no real wall time in the bounds in the fake data,
            # which rejects the test case, so only UTC is ever drawn.
            check_all(against_custom, lambda value: value.tzinfo is utc)
            against_default = generator(*IN_THE_FAKE_GAP, zones=zones, allow_imaginary=False)
            assert find_example(against_default, lambda value: value.tzinfo is london)
    assert repr(gs.temporal.zoned_datetimes(zones=wrapped[3], source=custom)) == (
        f"temporal.zoned_datetimes(zones={wrapped[3]!r}, source=ZoneSource())"
    )


def test_the_default_zones_are_those_of_the_given_source():
    london = zoneinfo.ZoneInfo("Europe/London")
    lo, hi = dt.datetime(2020, 6, 15, 11, 30), dt.datetime(2020, 6, 15, 12, 30)
    custom = ZoneSource()
    custom.add_tzif("Europe/London", FAKE_LONDON)
    zoned = gs.temporal.zoned_datetimes(lo, hi, source=custom)
    check_all(zoned, lambda value: value.tzinfo is london)
    assert repr(zoned) == (
        f"temporal.zoned_datetimes(min_value={lo!r}, max_value={hi!r}, source=ZoneSource())"
    )

    def body(tc):
        tc.draw(gs.temporal.zoned_datetimes(*IN_THE_FAKE_GAP, source=custom, allow_imaginary=False))

    assert "in any allowed zone" in str(failing(body, hegel.InvalidArgument))
    aware = gs.temporal.datetimes(lo.replace(tzinfo=UTC), hi.replace(tzinfo=UTC), source=custom)
    check_all(aware, lambda value: value.tzinfo is london)


def test_a_zone_the_source_lacks_is_an_error():
    custom = ZoneSource()
    custom.add_tzif("Europe/London", FAKE_LONDON)
    paris = zoneinfo.ZoneInfo("Europe/Paris")

    def body(tc):
        tc.draw(gs.temporal.zoned_datetimes(zones=gs.just(paris), source=custom))

    message = str(failing(body, hegel.InvalidArgument))
    assert "ZoneSource() has no time zone named 'Europe/Paris'" in message
    assert "source= to zoned_datetimes()" in message
    mars = zoneinfo.ZoneInfo.from_file(
        (files("tzdata.zoneinfo") / "UTC").open("rb"), key="Mars/Olympus_Mons"
    )

    def default_body(tc):
        tc.draw(gs.temporal.datetimes(zones=gs.just(mars)))

    message = str(failing(default_body, hegel.InvalidArgument))
    assert "ZoneSource.zoneinfo() has no time zone named 'Mars/Olympus_Mons'" in message
    # A name the source knows but cannot supply the data of is the
    # engine's error, not a missing name.
    declared = ZoneSource()
    declared.declare(["Europe/Paris"])

    def declared_body(tc):
        tc.draw(gs.temporal.datetimes(zones=gs.just(paris), source=declared))

    assert "has no time zone named" not in str(failing(declared_body, hegel.InvalidArgument))


def test_source_is_rejected_where_it_would_be_ignored():
    custom = ZoneSource()
    custom.add_tzif("Europe/London", FAKE_LONDON)
    with pytest.raises(hegel.InvalidArgument, match="pass source to zones\\(\\) instead"):
        gs.temporal.zoned_datetimes(zones=gs.temporal.zones(), source=custom)
    with pytest.raises(hegel.InvalidArgument, match="pass source to zones\\(\\) instead"):
        gs.temporal.datetimes(zones=gs.temporal.zones(source=custom), source=custom)
    with pytest.raises(hegel.InvalidArgument, match="draws naive datetimes unless"):
        gs.temporal.datetimes(source=custom)


def test_zones_from_a_source_zoneinfo_cannot_load():
    custom = ZoneSource()
    custom.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)

    def body(tc):
        tc.draw(gs.temporal.zones(source=custom))

    error = failing(body, hegel.InvalidArgument)
    assert "zoneinfo cannot load it" in str(error)
    with pytest.raises(hegel.InvalidArgument, match="not a time zone this Python can load"):
        gs.temporal.zones(names=["Europe/Olympus_Mons"], source=custom)


def test_a_source_from_tzif_bytes():
    source = ZoneSource()
    assert repr(source) == "ZoneSource()"
    assert source.version == ""
    source.version = "fake-1"
    assert source.version == "fake-1"
    source.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)
    source.add_link("Europe/Alias", "Europe/Olympus_Mons")
    names = gs.temporal.zone_names(source=source)
    check_all(names, lambda name: name in ("Europe/Olympus_Mons", "Europe/Alias"))
    assert find_example(names, lambda name: name == "Europe/Alias") == "Europe/Alias"
    alias = source.info("Europe/Alias")
    assert alias == ZoneNameInfo("Europe/Olympus_Mons", True, False, alias.rank)
    bundled = ZoneSource.bundled()
    assert alias.rank == source.info("Europe/Olympus_Mons").rank > bundled.info("UTC").rank == 0
    assert bundled.info("EST5EDT").is_legacy
    assert bundled.info("Asia/Calcutta") == ZoneNameInfo(
        "Asia/Kolkata", True, False, bundled.info("Asia/Calcutta").rank
    )
    assert source.info("Europe/Nowhere") is None
    assert "Europe/Alias" in source
    assert "Europe/Nowhere" not in source
    assert 5 not in source
    with pytest.raises(hegel.InvalidArgument, match="name=5 must be a str"):
        source.info(5)
    assert kind_of(source, "Europe/Alias", dt.datetime(2020, 6, 15, 12, 30)) == "gap"
    with pytest.raises(hegel.InvalidArgument, match="TZif"):
        source.add_tzif("Europe/Broken", b"not a TZif file")
    with pytest.raises(hegel.InvalidArgument, match="version=1 must be a str"):
        source.version = 1
    with pytest.raises(hegel.InvalidArgument, match="name=1 must be a str"):
        source.add_tzif(1, FAKE_LONDON)
    with pytest.raises(hegel.InvalidArgument, match="data='x' must be a bytes"):
        source.add_tzif("Europe/Olympus_Mons", "x")
    with pytest.raises(hegel.InvalidArgument, match="target=1 must be a str"):
        source.add_link("Europe/Alias", 1)


def london_transitions(*years):
    """The instants and types of London's transitions in ``years``."""
    transitions = []
    for year in years:
        march = max(day for day in range(25, 32) if dt.date(year, 3, day).weekday() == 6)
        october = max(day for day in range(25, 32) if dt.date(year, 10, day).weekday() == 6)
        transitions.append((timestamp(year, 3, march, 1), BST))
        transitions.append((timestamp(year, 10, october, 1), GMT))
    return transitions


def test_a_source_from_structured_data():
    source = ZoneSource()
    last_sunday = 6
    eu_rule = ZoneRule(
        GMT,
        BST,
        RuleDate.month_week_day(3, 5, last_sunday, time=HOUR),
        RuleDate.month_week_day(10, 5, last_sunday, time=2 * HOUR),
    )
    source.add_zone("Europe/Rule_EU", ZoneData(GMT, london_transitions(2019), rule=eu_rule))
    source.add_zone(
        "Europe/Rule_Inferred", ZoneData(GMT, london_transitions(2019, 2020), infer_after=2020)
    )
    source.add_zone("Europe/Rule_Held", ZoneData(GMT, london_transitions(2019, 2020)[:3]))
    source.add_zone(
        "Europe/Rule_Days",
        ZoneData(
            GMT,
            rule=ZoneRule(GMT, BST, RuleDate.julian(60), RuleDate.zero_based(300)),
        ),
    )
    source.add_zone("Europe/Rule_Fixed", ZoneData(GMT, rule=ZoneRule(BST)))
    gap = dt.datetime(2030, 3, 31, 1, 30)
    assert kind_of(source, "Europe/Rule_EU", gap) == "gap"
    assert kind_of(source, "Europe/Rule_Inferred", gap) == "gap"
    assert source.resolve("Europe/Rule_Held", gap).offset_after == HOUR
    assert kind_of(source, "Europe/Rule_Held", gap) == "unique"
    assert kind_of(source, "Europe/Rule_Days", dt.datetime(2030, 3, 1, 2, 30)) == "gap"
    assert source.resolve("Europe/Rule_Fixed", gap).offset_before == HOUR
    assert kind_of(source, "Europe/Rule_Fixed", gap) == "unique"
    check_all(
        gs.temporal.zone_names(source=source),
        lambda name: name.startswith("Europe/Rule_"),
    )
    with pytest.raises(hegel.InvalidArgument, match="ascending"):
        source.add_zone("Europe/Rule_Backwards", ZoneData(GMT, [(10, BST), (5, GMT)]))
    with pytest.raises(hegel.InvalidArgument, match="data=1 must be a ZoneData"):
        source.add_zone("Europe/Rule_Bad", 1)


@pytest.mark.parametrize(
    "build, message",
    [
        (lambda: ZoneType(1), "offset=1 must be a timedelta"),
        (lambda: ZoneType(dt.timedelta(microseconds=1)), "whole number of seconds"),
        (lambda: ZoneType(dt.timedelta(days=2)), "at most 1 day, 23:59:59 from zero"),
        (lambda: ZoneType(dt.timedelta(days=-2)), "at most 1 day, 23:59:59 from zero"),
        (lambda: ZoneType(HOUR, 1), "is_dst=1 must be a bool"),
        (lambda: ZoneType(HOUR, False, 1), "abbreviation=1 must be a str"),
        (lambda: RuleDate("weekly"), "kind='weekly' must be one of"),
        (lambda: RuleDate.month_week_day(13, 1, 0), "month=13 must be an int from 1 to 12"),
        (lambda: RuleDate.month_week_day(1, 6, 0), "week=6 must be an int from 1 to 5"),
        (lambda: RuleDate.month_week_day(1, 1, 7), "weekday=7 must be an int from 0 to 6"),
        (lambda: RuleDate.julian(0), "day=0 must be an int from 1 to 365"),
        (lambda: RuleDate.zero_based(366), "day=366 must be an int from 0 to 365"),
        (lambda: RuleDate.julian(1, time=dt.timedelta(hours=167, seconds=1)), "time="),
        (lambda: RuleDate.julian(1, time=dt.timedelta(hours=-168)), "time="),
        (lambda: RuleDate.julian(1, time=dt.timedelta(microseconds=1)), "time="),
        (lambda: ZoneRule(1), "standard=1 must be a ZoneType"),
        (lambda: ZoneRule(GMT, dst_start=RuleDate.julian(1)), "need a dst type"),
        (lambda: ZoneRule(GMT, dst_end=RuleDate.julian(1)), "need a dst type"),
        (lambda: ZoneRule(GMT, 1), "dst=1 must be a ZoneType"),
        (lambda: ZoneRule(GMT, BST), "dst_start=None must be a RuleDate"),
        (lambda: ZoneRule(GMT, BST, RuleDate.julian(1)), "dst_end=None must be a RuleDate"),
        (lambda: ZoneData(1), "initial=1 must be a ZoneType"),
        (lambda: ZoneData(GMT, "abc"), "must be a sequence"),
        (lambda: ZoneData(GMT, 5), "must be a sequence"),
        (lambda: ZoneData(GMT, [1]), "must be an \\(int, ZoneType\\) pair"),
        (lambda: ZoneData(GMT, [(1, 2, 3)]), "must be an \\(int, ZoneType\\) pair"),
        (lambda: ZoneData(GMT, [(1, 2)]), "must be an \\(int, ZoneType\\) pair"),
        (lambda: ZoneData(GMT, [(2**63, BST)]), "instant=9223372036854775808"),
        (lambda: ZoneData(GMT, [(True, BST)]), "instant=True"),
        (lambda: ZoneData(GMT, rule=1), "rule=1 must be a ZoneRule"),
        (lambda: ZoneData(GMT, rule=ZoneRule(GMT), infer_after=2020), "not both"),
        (lambda: ZoneData(GMT, infer_after=10**6), "infer_after=1000000"),
        (lambda: ZoneOracle(1, first_year=1, last_year=2, after="hold"), "offset_at=1"),
        (
            lambda: ZoneOracle(print, first_year=1, last_year=2, after="hold", next_transition=1),
            "next_transition=1",
        ),
        (lambda: ZoneOracle(print, first_year="1", last_year=2, after="hold"), "first_year='1'"),
        (lambda: ZoneOracle(print, first_year=1, last_year=10**6, after="hold"), "last_year="),
        (lambda: ZoneOracle(print, first_year=1, last_year=2, after="rule"), "after='rule'"),
    ],
)
def test_structured_data_validation(build, message):
    with pytest.raises(hegel.InvalidArgument, match=message):
        build()


def test_rule_times_reach_the_limits_a_tzif_file_allows():
    # RFC 9636 allows a rule's time from -167 to 167 hours, both included.
    source = ZoneSource()
    limit = dt.timedelta(hours=167)
    rule = ZoneRule(GMT, BST, RuleDate.julian(60, time=limit), RuleDate.julian(300, time=-limit))
    source.add_zone("Europe/Rule_Limits", ZoneData(GMT, rule=rule))
    # Day 60 is March 1st, and 167 hours after its midnight is 23:00 on
    # March 7th, when the clocks go forward an hour.
    assert kind_of(source, "Europe/Rule_Limits", dt.datetime(2030, 3, 7, 23, 30)) == "gap"
    assert kind_of(source, "Europe/Rule_Limits", dt.datetime(2030, 3, 7, 22, 30)) == "unique"
    edge = ZoneType(dt.timedelta(days=2) - dt.timedelta(seconds=1))
    source.add_zone("Europe/Rule_Edge", ZoneData(edge))
    assert source.resolve("Europe/Rule_Edge", dt.datetime(2030, 1, 1)).offset_before == edge.offset


def test_source_queries():
    source = ZoneSource()
    source.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)
    forward = dt.datetime(2020, 6, 15, 12, tzinfo=UTC)
    back = dt.datetime(2020, 9, 1, 11, tzinfo=UTC)
    assert source.offset_at("Europe/Olympus_Mons", forward - HOUR) == ZoneType(
        dt.timedelta(0), False, "XMT"
    )
    assert source.offset_at("Europe/Olympus_Mons", forward) == ZoneType(HOUR, True, "XST")
    in_new_york = (back - HOUR).astimezone(zoneinfo.ZoneInfo("America/New_York"))
    assert source.offset_at("Europe/Olympus_Mons", in_new_york).abbreviation == "XST"

    unique = source.resolve("Europe/Olympus_Mons", dt.datetime(2020, 7, 1, 9))
    assert (unique.kind, unique.offset_before, unique.offset_after) == ("unique", HOUR, HOUR)
    assert unique.instant == dt.datetime(2020, 7, 1, 8, tzinfo=UTC)
    # 12:00 to 12:59 was skipped: read with the offset before the change
    # it is an hour later on the clock, and with the one after it an hour
    # earlier, as datetime reads fold 0 and fold 1.
    skipped = dt.datetime(2020, 6, 15, 12, 30)
    gap = source.resolve("Europe/Olympus_Mons", skipped)
    assert (gap.kind, gap.offset_before, gap.offset_after) == ("gap", dt.timedelta(0), HOUR)
    assert gap.instant == forward + dt.timedelta(minutes=30)
    assert source.resolve("Europe/Olympus_Mons", skipped.replace(fold=1)).instant == (
        forward - dt.timedelta(minutes=30)
    )
    # 11:00 to 11:59 on September 1st happened twice.
    repeated = dt.datetime(2020, 9, 1, 11, 30)
    fold = source.resolve("Europe/Olympus_Mons", repeated)
    assert (fold.kind, fold.offset_before, fold.offset_after) == ("fold", HOUR, dt.timedelta(0))
    assert fold.instant == back - dt.timedelta(minutes=30)
    assert source.resolve("Europe/Olympus_Mons", repeated.replace(fold=1)).instant == (
        back + dt.timedelta(minutes=30)
    )

    with pytest.raises(hegel.InvalidArgument, match='unknown time zone "Europe/Nowhere"'):
        source.resolve("Europe/Nowhere", skipped)
    with pytest.raises(hegel.InvalidArgument, match='unknown time zone "Europe/Nowhere"'):
        source.offset_at("Europe/Nowhere", forward)
    with pytest.raises(hegel.InvalidArgument, match="wall_time=.* must be naive"):
        source.resolve("Europe/Olympus_Mons", forward)
    with pytest.raises(hegel.InvalidArgument, match="wall_time=.* must be a datetime"):
        source.resolve("Europe/Olympus_Mons", dt.date(2020, 1, 1))
    with pytest.raises(hegel.InvalidArgument, match="instant=.* must be aware"):
        source.offset_at("Europe/Olympus_Mons", skipped)
    with pytest.raises(hegel.InvalidArgument, match="instant=5 must be a datetime"):
        source.offset_at("Europe/Olympus_Mons", 5)
    with pytest.raises(hegel.InvalidArgument, match="name=5 must be a str"):
        source.resolve(5, skipped)
    with pytest.raises(hegel.InvalidArgument, match="name=5 must be a str"):
        source.offset_at(5, forward)
    # The first wall times of year 1 in a zone ahead of UTC are moments in
    # the year before, which datetime cannot hold.
    tokyo = ZoneSource.bundled()
    with pytest.raises(hegel.InvalidArgument, match="outside the range datetime represents"):
        tokyo.resolve("Asia/Tokyo", dt.datetime.min)
    check_all(
        gs.temporal.zoned_datetimes(dt.datetime(1990, 1, 1), dt.datetime(2030, 1, 1)),
        lambda value: (
            gs.temporal.default_zone_source().offset_at(value.tzinfo.key, value).offset
            == value.utcoffset()
            or _is_imaginary(value)
        ),
        test_cases=100,
    )


def offset_at_of(zone):
    def offset_at(instant):
        moment = dt.datetime.fromtimestamp(instant, UTC).astimezone(zone)
        return ZoneType(moment.utcoffset(), bool(moment.dst()), moment.tzname())

    return offset_at


def test_a_source_from_an_oracle():
    london = zoneinfo.ZoneInfo("Europe/London")
    source = ZoneSource()
    source.add_oracle(
        "Europe/Oracle_Sampled",
        ZoneOracle(offset_at_of(london), first_year=2018, last_year=2020, after="infer"),
    )
    transitions = [instant for instant, _ in london_transitions(2018, 2019, 2020)]

    def next_transition(after):
        return next((instant for instant in transitions if instant > after), None)

    source.add_oracle(
        "Europe/Oracle_Walked",
        ZoneOracle(
            offset_at_of(london),
            first_year=2018,
            last_year=2020,
            after="hold",
            next_transition=next_transition,
        ),
    )
    assert kind_of(source, "Europe/Oracle_Sampled", dt.datetime(2030, 3, 31, 1, 30)) == "gap"
    assert kind_of(source, "Europe/Oracle_Walked", dt.datetime(2019, 3, 31, 1, 30)) == "gap"
    held = source.resolve("Europe/Oracle_Walked", dt.datetime(2030, 3, 31, 1, 30))
    assert (held.kind, held.offset_before) == ("unique", dt.timedelta(0))
    with pytest.raises(hegel.InvalidArgument, match="oracle=1 must be a ZoneOracle"):
        source.add_oracle("Europe/Oracle_Bad", 1)


def test_an_oracle_that_fails():
    source = ZoneSource()

    def broken(instant):
        raise ZeroDivisionError("no offsets here")

    with pytest.raises(hegel.InvalidArgument, match="ZeroDivisionError: no offsets here") as info:
        source.add_oracle(
            "Europe/Oracle_Broken",
            ZoneOracle(broken, first_year=2019, last_year=2020, after="hold"),
        )
    assert isinstance(info.value.__cause__, ZeroDivisionError)
    with pytest.raises(hegel.InvalidArgument, match="not a ZoneType") as info:
        source.add_oracle(
            "Europe/Oracle_Wrong",
            ZoneOracle(lambda instant: 0, first_year=2019, last_year=2020, after="hold"),
        )
    assert isinstance(info.value.__cause__, hegel.InvalidArgument)

    def no_next(after):
        raise ValueError("lost")

    with pytest.raises(hegel.InvalidArgument, match="ValueError: lost") as info:
        source.add_oracle(
            "Europe/Oracle_Lost",
            ZoneOracle(
                lambda instant: GMT,
                first_year=2019,
                last_year=2020,
                after="hold",
                next_transition=no_next,
            ),
        )
    assert isinstance(info.value.__cause__, ValueError)


def test_a_lazily_loaded_source():
    calls = []
    entries = {
        "Europe/London": FAKE_LONDON,
        "Europe/Paris": ZoneData(GMT, london_transitions(2019, 2020), infer_after=2020),
        "Europe/Berlin": ZoneOracle(
            offset_at_of(zoneinfo.ZoneInfo("Europe/London")),
            first_year=2019,
            last_year=2020,
            after="infer",
        ),
        "Europe/Belfast": "Europe/London",
    }

    def loader(name):
        calls.append(name)
        return entries.get(name)

    source = ZoneSource()
    source.declare(entries)
    source.set_loader(loader)
    names = gs.temporal.zone_names(source=source)
    check_all(names, lambda name: name in entries)
    assert calls == []
    assert kind_of(source, "Europe/Belfast", dt.datetime(2020, 6, 15, 12, 30)) == "gap"
    assert calls == ["Europe/Belfast", "Europe/London"]
    for name in ("Europe/Paris", "Europe/Berlin"):
        assert kind_of(source, name, dt.datetime(2030, 3, 31, 1, 30)) == "gap"
        assert kind_of(source, name, dt.datetime(2030, 3, 31, 3, 30)) == "unique"
    assert calls == ["Europe/Belfast", "Europe/London", "Europe/Paris", "Europe/Berlin"]
    assert "Europe/Lazy_Missing" not in source
    assert "Europe/Lazy_Missing" not in source
    assert calls.count("Europe/Lazy_Missing") == 1
    source.set_loader(None)
    with pytest.raises(hegel.InvalidArgument, match="loader=1 must be callable"):
        source.set_loader(1)
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of strings"):
        source.declare("Europe/London")
    with pytest.raises(hegel.InvalidArgument, match="name=1 must be a str"):
        source.declare([1])


class CountingLoader:
    def __init__(self, entries):
        self.entries = entries

    def __call__(self, name):
        return self.entries.get(name)


def test_a_loader_is_released_when_the_source_drops_it():
    def loader_of(source):
        loader = CountingLoader({"Europe/London": FAKE_LONDON})
        source.set_loader(loader)
        return weakref.ref(loader)

    source = ZoneSource()
    first = loader_of(source)
    assert kind_of(source, "Europe/London", dt.datetime(2020, 6, 15, 12, 30)) == "gap"
    gc.collect()
    assert first() is not None
    second = loader_of(source)
    gc.collect()
    assert first() is None
    assert second() is not None
    source.set_loader(None)
    gc.collect()
    assert second() is None

    third = loader_of(source)
    names = gs.temporal.zone_names(names=["Europe/London"], source=source)
    del source
    gc.collect()
    # The generator still draws from the source, which keeps its loader.
    assert third() is not None
    del names
    gc.collect()
    assert third() is None

    # A source the engine refuses a loader for releases it at once.
    refused = loader_of_refused()
    gc.collect()
    assert refused() is None


def loader_of_refused():
    loader = CountingLoader({})
    with pytest.raises(hegel.InvalidArgument):
        ZoneSource.bundled().set_loader(loader)
    return weakref.ref(loader)


def test_a_loader_may_fill_the_source_it_is_loading_for():
    source = ZoneSource()

    def loader(name):
        if name == "Europe/London":
            source.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)
            source.add_link("Europe/Belfast", "Europe/London")
            assert "Europe/Olympus_Mons" in source
            return FAKE_LONDON
        return None

    source.declare(["Europe/London"])
    source.set_loader(loader)
    assert kind_of(source, "Europe/London", dt.datetime(2020, 6, 15, 12, 30)) == "gap"
    assert kind_of(source, "Europe/Belfast", dt.datetime(2020, 6, 15, 12, 30)) == "gap"
    check_all(
        gs.temporal.zone_names(source=source),
        lambda name: name in ("Europe/London", "Europe/Olympus_Mons", "Europe/Belfast"),
    )


def test_a_loader_that_fails_fails_the_draw():
    def loader(name):
        raise ZeroDivisionError(name)

    source = ZoneSource()
    source.declare(["Europe/London"])
    source.set_loader(loader)
    zoned = gs.temporal.zoned_datetimes(zones=gs.temporal.zones(source=source))

    def body(tc):
        tc.draw(zoned)

    error = failing(body, hegel.InvalidArgument)
    assert "ZeroDivisionError: Europe/London" in str(error)
    assert isinstance(error.__cause__, ZeroDivisionError)

    source.set_loader(lambda name: 42)
    with pytest.raises(hegel.InvalidArgument, match="the loader returned 42") as info:
        source.resolve("Europe/Lazy_Other", dt.datetime(2020, 1, 1))
    assert isinstance(info.value.__cause__, hegel.InvalidArgument)

    def interrupted(name):
        raise KeyboardInterrupt

    source.set_loader(interrupted)
    with pytest.raises(KeyboardInterrupt):
        gs.temporal.zone_names(names=["Europe/Lazy_Interrupted"], source=source)


def test_a_slow_loader_does_not_block_other_threads():
    def loader(name):
        time.sleep(0.01)
        return FAKE_LONDON

    source = ZoneSource()
    source.declare(sorted(zoneinfo.available_timezones())[:40])
    source.set_loader(loader)
    spec = gs.temporal.zone_names(source=source).spec
    zoned_spec = _native.ZonedSpec(
        ((2000, 1, 1, 0, 0, 0, 0), (2030, 1, 1, 0, 0, 0, 0)),
        None,
        _native.PRECISION_SECONDS,
        True,
        _native.AMBIGUOUS_BOTH,
    )

    def body(tc):
        errors = []

        def draw(worker):
            try:
                for _ in range(3):
                    worker._generate_zoned_datetime(spec, zoned_spec)
            except BaseException as error:
                errors.append(error)

        threads = [threading.Thread(target=draw, args=(tc.clone(),), daemon=True) for _ in range(4)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join(timeout=10)
        assert not any(thread.is_alive() for thread in threads)
        assert errors == []

    run_test(body, test_cases=5)
