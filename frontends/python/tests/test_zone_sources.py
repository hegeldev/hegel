from __future__ import annotations

import datetime as dt
import struct
import sys
import threading
import time
import zoneinfo
from importlib.resources import files

import pytest

import hegel
from hegel import _native
from hegel import generators as gs
from hegel.generators.temporal import (
    RuleDate,
    ZoneData,
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
    yield
    gs.temporal.set_default_zone_source(previous)


def resolve(source, name, civil):
    """The engine's ``(kind, offset)`` for the naive datetime ``civil`` in
    the zone ``name`` of ``source``."""
    kind, _, _, offset, _, _ = _native.timezone_resolve(
        name, (*civil.timetuple()[:6], civil.microsecond * 1000), source._handle()
    )
    return kind, offset


def check_all(generator, predicate, *, test_cases=50):
    def body(tc):
        value = tc.draw(generator)
        assert predicate(value), repr(value)

    run_test(body, test_cases=test_cases)


def agrees_with_zoneinfo(value, source):
    kind, before, after, offset, _, _ = _native.timezone_resolve(
        value.tzinfo.key, (*value.timetuple()[:6], value.microsecond * 1000), source._handle()
    )
    if kind == _native.TIMEZONE_KIND_FOLD:
        offset = after if value.fold else before
    elif kind == _native.TIMEZONE_KIND_GAP:
        offset = before
    return value.utcoffset() == dt.timedelta(seconds=offset)


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
    assert resolve(source, "Europe/London", dt.datetime(2020, 6, 15, 12, 30))[0] == (
        _native.TIMEZONE_KIND_GAP
    )
    assert resolve(source, "Europe/London", dt.datetime(2020, 3, 29, 1, 30))[0] == (
        _native.TIMEZONE_KIND_UNIQUE
    )
    real = ZoneSource.bundled()
    assert resolve(real, "Europe/London", dt.datetime(2020, 6, 15, 12, 30))[0] == (
        _native.TIMEZONE_KIND_UNIQUE
    )
    in_london = gs.temporal.zones(source=source, no_cache=True)
    june = gs.temporal.zoned_datetimes(
        dt.datetime(2020, 6, 15), dt.datetime(2020, 6, 16), zones=in_london
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
        dt.datetime(2020, 9, 1), dt.datetime(2020, 9, 2), zones=in_london
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
    assert (
        repr(names) == "temporal.zone_names(links=True, legacy=False, source=ZoneSource.zoneinfo())"
    )
    with pytest.raises(hegel.InvalidArgument, match="'zone.tab' is not a time zone name"):
        gs.temporal.zone_names(names=["zone.tab"], source=source)
    with pytest.raises(hegel.InvalidArgument, match="'Europe' is not a time zone name"):
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
    with pytest.raises(hegel.InvalidArgument, match="install the tzdata package"):
        gs.temporal.zone_names(source=ZoneSource.zoneinfo())


def test_the_bundled_source():
    source = ZoneSource.bundled()
    assert repr(source) == "ZoneSource.bundled()"
    assert source.version == gs.temporal.tzdb_version()
    zones = gs.temporal.zones(source=source)
    assert repr(zones) == (
        "temporal.zones(links=True, legacy=False, source=ZoneSource.bundled(), no_cache=False)"
    )
    check_all(zones, lambda zone: isinstance(zone, zoneinfo.ZoneInfo))
    with pytest.raises(hegel.InvalidArgument):
        source.version = "2000a"
    with pytest.raises(hegel.InvalidArgument):
        source.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)


def test_setting_the_default_source(default_source):
    custom = ZoneSource()
    custom.add_tzif("Europe/Olympus_Mons", FAKE_LONDON)
    gs.temporal.set_default_zone_source(custom)
    assert gs.temporal.default_zone_source() is custom
    check_all(gs.temporal.zone_names(), lambda name: name == "Europe/Olympus_Mons")
    assert repr(gs.temporal.zone_names()) == "temporal.zone_names(links=True, legacy=False)"
    with pytest.raises(hegel.InvalidArgument, match="source=None must be a ZoneSource"):
        gs.temporal.set_default_zone_source(None)
    with pytest.raises(hegel.InvalidArgument, match="source='bundled' must be a ZoneSource"):
        gs.temporal.zone_names(source="bundled")


def test_a_zoneinfo_zone_from_another_generator_uses_the_default_source(default_source):
    london = zoneinfo.ZoneInfo("Europe/London")
    lo, hi = dt.datetime(2020, 6, 15), dt.datetime(2020, 6, 16)
    custom = ZoneSource()
    custom.add_tzif("Europe/London", FAKE_LONDON)
    gs.temporal.set_default_zone_source(custom)
    gap = find_example(gs.temporal.datetimes(lo, hi, zones=gs.just(london)), lambda d: d.hour == 12)
    assert resolve(custom, "Europe/London", gap.replace(tzinfo=None))[0] == (
        _native.TIMEZONE_KIND_GAP
    )


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
    assert _native.timezone_info("Europe/Alias", source._handle())[:2] == (
        "Europe/Olympus_Mons",
        True,
    )
    assert resolve(source, "Europe/Alias", dt.datetime(2020, 6, 15, 12, 30))[0] == (
        _native.TIMEZONE_KIND_GAP
    )
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
    assert resolve(source, "Europe/Rule_EU", gap)[0] == _native.TIMEZONE_KIND_GAP
    assert resolve(source, "Europe/Rule_Inferred", gap)[0] == _native.TIMEZONE_KIND_GAP
    assert resolve(source, "Europe/Rule_Held", gap) == (_native.TIMEZONE_KIND_UNIQUE, 3600)
    assert resolve(source, "Europe/Rule_Days", dt.datetime(2030, 3, 1, 2, 30))[0] == (
        _native.TIMEZONE_KIND_GAP
    )
    assert resolve(source, "Europe/Rule_Fixed", gap) == (_native.TIMEZONE_KIND_UNIQUE, 3600)
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
        (lambda: ZoneType(dt.timedelta(days=2)), "whole number of seconds"),
        (lambda: ZoneType(HOUR, 1), "is_dst=1 must be a bool"),
        (lambda: ZoneType(HOUR, False, 1), "abbreviation=1 must be a str"),
        (lambda: RuleDate("weekly"), "kind='weekly' must be one of"),
        (lambda: RuleDate.month_week_day(13, 1, 0), "month=13 must be an int from 1 to 12"),
        (lambda: RuleDate.month_week_day(1, 6, 0), "week=6 must be an int from 1 to 5"),
        (lambda: RuleDate.month_week_day(1, 1, 7), "weekday=7 must be an int from 0 to 6"),
        (lambda: RuleDate.julian(0), "day=0 must be an int from 1 to 365"),
        (lambda: RuleDate.zero_based(366), "day=366 must be an int from 0 to 365"),
        (lambda: RuleDate.julian(1, time=dt.timedelta(hours=167)), "time="),
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
    assert resolve(source, "Europe/Oracle_Sampled", dt.datetime(2030, 3, 31, 1, 30))[0] == (
        _native.TIMEZONE_KIND_GAP
    )
    assert resolve(source, "Europe/Oracle_Walked", dt.datetime(2019, 3, 31, 1, 30))[0] == (
        _native.TIMEZONE_KIND_GAP
    )
    assert resolve(source, "Europe/Oracle_Walked", dt.datetime(2030, 3, 31, 1, 30)) == (
        _native.TIMEZONE_KIND_UNIQUE,
        0,
    )
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
    assert resolve(source, "Europe/Belfast", dt.datetime(2020, 6, 15, 12, 30))[0] == (
        _native.TIMEZONE_KIND_GAP
    )
    assert calls == ["Europe/Belfast", "Europe/London"]
    for name in ("Europe/Paris", "Europe/Berlin"):
        assert resolve(source, name, dt.datetime(2030, 3, 31, 1, 30))[0] == (
            _native.TIMEZONE_KIND_GAP
        )
        assert resolve(source, name, dt.datetime(2030, 3, 31, 3, 30))[0] == (
            _native.TIMEZONE_KIND_UNIQUE
        )
    assert calls == ["Europe/Belfast", "Europe/London", "Europe/Paris", "Europe/Berlin"]
    assert _native.timezone_info("Europe/Lazy_Missing", source._handle()) is None
    assert _native.timezone_info("Europe/Lazy_Missing", source._handle()) is None
    assert calls.count("Europe/Lazy_Missing") == 1
    source.set_loader(None)
    with pytest.raises(hegel.InvalidArgument, match="loader=1 must be callable"):
        source.set_loader(1)
    with pytest.raises(hegel.InvalidArgument, match="must be an iterable of strings"):
        source.declare("Europe/London")
    with pytest.raises(hegel.InvalidArgument, match="name=1 must be a str"):
        source.declare([1])


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
        resolve(source, "Europe/Lazy_Other", dt.datetime(2020, 1, 1))
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
