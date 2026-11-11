# Zone sources: generating against the time zone data the code under test uses

Status: implemented in the engine (2026-10-23) and adopted by every
frontend. Supersedes the version-skew contract of `datetime.md` §8.1. The
Summary and §1 describe the problem as it stood before zone sources.

## Summary

Today every frontend draws zoned values against the engine's bundled IANA
database (`src/tzdata/`, one pinned release), and the host library then
decides what the value means. When the two disagree, the value is still
valid but misses the edge it was aimed at, and the Java, Python and
TypeScript frontends reject the test case outright. The gaps and folds
Hegel targets are the engine's, not the host's. The disagreements cluster
in recently changed zones, which is exactly where real bugs live.

This proposal adds **zone sources**. A zone source supplies a zone's data
(its transitions and its rule for the future) from whatever database the
code under test actually uses. That might be `zoneinfo`, `java.time`,
jiff, chrono-tz, Boost.DateTime or the browser's `Intl`. The engine then
classifies, targets and resolves wall times against that data. The bundled
database stays as two things:

- **the default source**, for frontends and libraries with no database of
  their own;
- **the catalogue**: the source of zone names, link and legacy flags,
  feature tags and the shrink ladder, which stay the same on every host.

This plays the role the host Unicode version plays for strings
(`hegel_set_unicode_version`). Each frontend sets a **process-wide default
source**: its standard library's database where the language ships one
(Python `zoneinfo`, Java `java.time`, JavaScript `Temporal`/`Intl`), and
the engine's bundled database where it does not (Rust, C++). Process-wide
alone is not enough, though. One Rust test can draw a jiff `Zoned` (the
system database, or `jiff-tzdb`), a chrono-tz `DateTime` (chrono-tz's
compiled-in release) and a core `hegel` value (the bundled release) side
by side, and a C++ program can mix `std::chrono` and Boost. So a zone draw
can also carry its own source, which overrides the default, and the
generators of a library integration always attach that library's source.

## 1. What the engine uses zone data for

| Use | Data | Where | Must match the host? |
|---|---|---|---|
| Which names exist; link and legacy flags | name list, `backward` links | `tzdata::names()` | Names: yes, already handled by explicit name lists (§8.1). Flags: no. |
| Draw weighting, shrink ladder | feature tags, rank | `Name::tags`, `Name::rank` | No. Better stable across hosts, so a stored failure shrinks the same way everywhere. |
| Targeting transitions | explicit transitions in the window | `transitions()` in `draws/zoned.rs`, `draws/scalar.rs` | Yes: this is the point. |
| Classifying a wall time (unique, gap, fold) and resolving it to an instant | initial type, transitions, footer rule | `native::timezone::{type_at, resolve}` | Yes. |
| Abbreviations, `is_dst` (`offset_at`, `info` queries) | types | `native::timezone` | Only for the query functions. |

So the split is clean. The **catalogue** (names, flags, tags, rank) stays
bundled and keyed by name. The **zone data** (types, transitions, footer)
comes from the source.

One consequence: a name the catalogue lacks but the source knows becomes a
real zone, rather than today's opaque one. That covers a zone newer than
the engine, or a host-specific name such as Boost's or a test fixture's.
It gets the last rank, and tags computed from its data by the tag
classifier, ported from `generate_tzdata.py` into the engine (§6).

## 2. The data model

A source answers one question: given a name, what is this zone's data, if
it knows the zone? The answer is a `Zone`, the structure the engine already
uses internally (RFC 9636's TZif model):

- an initial local time type (offset, `is_dst`, abbreviation);
- explicit transitions (instant, type);
- an optional footer: a standard type plus an optional DST rule, applying
  after the last explicit transition.

Frontends can hand it over in three shapes, because their libraries expose
different amounts:

1. **TZif bytes.** The engine parses them; the parser is a Rust port of
   `parse_tzif`/`parse_footer` in `generate_tzdata.py`. Suits libraries
   that read TZif files themselves: Python `zoneinfo`, jiff (system
   database or `jiff-tzdb`), and TZif files on disk in general.
2. **Structured data**: explicit transitions plus a footer rule given as
   fields, not text. Suits libraries that expose their rules:
   - `java.time`: `ZoneRules.getTransitions()` plus
     `getTransitionRules()`, mapped to footer rules.
   - Boost.DateTime: `posix_time_zone` or `custom_time_zone`, which is a
     footer with no explicit transitions. Boost holds only each zone's
     current rule, so the zone has no history. That is what Boost
     computes too, so generating against it is right.
3. **An offset oracle**: "what is the offset at instant t", optionally with
   "when is the next transition after t". Suits libraries that
   expose only lookups:
   - chrono-tz, through chrono's `TimeZone::offset_from_utc_datetime`;
     its transitions are compiled into private tables.
   - `Intl.DateTimeFormat` in JavaScript.
   - Temporal's `getTimeZoneTransition`.
   - `std::chrono::time_zone::get_info` in C++20 (`sys_info` begin and
     end).

   The engine turns an oracle into explicit transitions over a bounded
   probing range. Beyond that range it holds the last offset or infers a
   footer, as the source chooses (§6). It only
   probes zones actually drawn, so the cost scales with use.

## 3. Engine API

Implemented in `src/native/zone_source.rs` (the sources), `src/tzdata/tzif.rs`
(the TZif and POSIX TZ parsers), `src/tzdata/classify.rs` (the tag
classifier) and `src/lib.rs` (the C ABI). `include/hegel.h` documents every
function and struct; this is the outline.

```c
typedef struct hegel_zone_source_t hegel_zone_source_t;

/* an empty custom source, and a handle on the bundled database */
hegel_result_t hegel_zone_source_new(hegel_context_t*, hegel_zone_source_t** out_source);
hegel_result_t hegel_zone_source_bundled(hegel_context_t*, hegel_zone_source_t** out_source);
hegel_result_t hegel_zone_source_free(hegel_context_t*, hegel_zone_source_t*);

/* shape 1: a TZif file (64-bit block and footer) */
hegel_result_t hegel_zone_source_add_tzif(hegel_context_t*, hegel_zone_source_t*,
                                          hegel_string_t name,
                                          const uint8_t* bytes, size_t len);

/* shape 2: structured data */
typedef struct { int32_t utoff; bool is_dst; hegel_string_t abbreviation; } hegel_zone_type_t;
typedef struct { int64_t instant; hegel_zone_type_t local_type; } hegel_zone_transition_t;
typedef enum { HEGEL_ZONE_RULE_DAY_JULIAN, HEGEL_ZONE_RULE_DAY_ZERO_BASED,
               HEGEL_ZONE_RULE_DAY_MONTH_WEEK_DAY } hegel_zone_rule_day_t;
typedef struct { uint32_t kind; uint16_t day; uint8_t month, week, weekday;
                 int32_t time; } hegel_zone_rule_date_t;
typedef enum { HEGEL_ZONE_AFTER_HOLD, HEGEL_ZONE_AFTER_RULE,
               HEGEL_ZONE_AFTER_INFER } hegel_zone_after_t;
typedef struct {
    hegel_zone_type_t initial;
    const hegel_zone_transition_t* transitions; size_t transition_count;
    uint32_t after;                         /* hegel_zone_after_t */
    hegel_zone_type_t standard;             /* HEGEL_ZONE_AFTER_RULE */
    bool has_dst; hegel_zone_type_t dst;
    hegel_zone_rule_date_t dst_start, dst_end;
    int32_t last_year;                      /* HEGEL_ZONE_AFTER_INFER */
} hegel_zone_data_t;
hegel_result_t hegel_zone_source_add_zone(hegel_context_t*, hegel_zone_source_t*,
                                          hegel_string_t name, const hegel_zone_data_t*);

/* shape 3: an offset oracle, probed during the call; a callback returns 0
   for success and anything else for failure */
typedef int32_t (*hegel_zone_offset_fn_t)(void* user_data, hegel_context_t* ctx,
                                          int64_t instant, hegel_zone_type_t* out_type);
typedef int32_t (*hegel_zone_next_transition_fn_t)(void* user_data, hegel_context_t* ctx,
                                                   int64_t after, bool* out_found,
                                                   int64_t* out_instant);
typedef struct {
    hegel_zone_offset_fn_t offset_at;               /* required */
    hegel_zone_next_transition_fn_t next_transition; /* optional */
    void* user_data;
    int32_t first_year, last_year;
    uint32_t after;              /* HEGEL_ZONE_AFTER_HOLD or HEGEL_ZONE_AFTER_INFER */
} hegel_zone_oracle_t;
hegel_result_t hegel_zone_source_add_oracle(hegel_context_t*, hegel_zone_source_t*,
                                            hegel_string_t name, const hegel_zone_oracle_t*);

/* names: links, and names whose data comes later */
hegel_result_t hegel_zone_source_add_link(hegel_context_t*, hegel_zone_source_t*,
                                          hegel_string_t name, hegel_string_t target);
hegel_result_t hegel_zone_source_declare(hegel_context_t*, hegel_zone_source_t*,
                                         const hegel_string_t* names, size_t len);
typedef int32_t (*hegel_zone_loader_t)(void* user_data, hegel_context_t* ctx,
                                       hegel_zone_source_t* source, hegel_string_t name);
typedef void (*hegel_zone_loader_release_t)(void* user_data);
hegel_result_t hegel_zone_source_set_loader(hegel_context_t*, hegel_zone_source_t*,
                                            hegel_zone_loader_t, void* user_data,
                                            hegel_zone_loader_release_t release);

/* the names a source knows, in ascending byte order */
hegel_result_t hegel_zone_source_name_count(hegel_context_t*, const hegel_zone_source_t*,
                                            size_t* out_count);
hegel_result_t hegel_zone_source_name(hegel_context_t*, const hegel_zone_source_t*,
                                      size_t index, hegel_string_t* out_name);

hegel_result_t hegel_zone_source_set_version(hegel_context_t*, hegel_zone_source_t*,
                                             hegel_string_t version);
hegel_result_t hegel_zone_source_version(hegel_context_t*, const hegel_zone_source_t*,
                                         hegel_string_t* out_version);

/* for callbacks: the message a failing callback reports */
hegel_result_t hegel_context_set_error(hegel_context_t*, hegel_string_t message);

/* the process-wide default; NULL restores the bundled database */
hegel_result_t hegel_set_default_zone_source(hegel_context_t*, const hegel_zone_source_t*);

/* attach to a draw, overriding the default; NULL means the default as it is now */
hegel_result_t hegel_timezone_spec_set_source(hegel_context_t*, hegel_timezone_spec_t*,
                                              const hegel_zone_source_t*);
/* check a spec against its source, as its first draw would */
hegel_result_t hegel_timezone_spec_validate(hegel_context_t*, const hegel_timezone_spec_t*);

/* a zoned datetime in a zone the caller names; NULL means the default source */
hegel_result_t hegel_generate_zoned_datetime_for_zone(hegel_context_t*, hegel_test_case_t*,
                                                      const hegel_zone_source_t*,
                                                      hegel_string_t name,
                                                      const hegel_zoned_spec_t*,
                                                      hegel_zoned_datetime_t* out_value);

/* the queries against a source; the existing ones pass NULL, the default */
hegel_result_t hegel_timezone_resolve_in(hegel_context_t*, const hegel_zone_source_t*,
                                         hegel_string_t name, hegel_datetime_t civil,
                                         hegel_timezone_resolution_t* out);
hegel_result_t hegel_timezone_offset_at_in(hegel_context_t*, const hegel_zone_source_t*,
                                           hegel_string_t name, hegel_timestamp_t timestamp,
                                           hegel_timezone_offset_t* out);
hegel_result_t hegel_timezone_info_in(hegel_context_t*, const hegel_zone_source_t*,
                                      hegel_string_t name, hegel_timezone_info_t* out);
```

### 3.1 Sources and their data

- **Handles.** A handle shares its source: freeing it leaves the source
  alive while the default or a spec holds it, and the loader receives a
  handle on the source it loads into. The bundled source is a source like
  any other for reading; every change to it is an argument error.
- **Checks.** Whatever the shape, the data is checked: every offset within
  two days of UT, transitions strictly ascending, rule dates as RFC 9636
  §3.3.1 allows, abbreviations without NUL bytes, names non-empty, without
  NUL bytes and not `Etc/Unknown`. A rule must continue the explicit
  transitions: the type the last transition changes to has to be the one
  the rule has in force at that instant, since anything else would give the
  zone two answers there. A rule whose daylight type equals its standard
  type is the standard type alone. The data is then confined to the
  engine's civil envelope (a transition before it folds into the initial
  type; one after it ends the history there and drops the rule), pruned of
  records that change nothing as `generate_tzdata.py` prunes the bundled
  table, and its abbreviations renumbered in the script's order. A TZif
  file of the same release as the bundled table therefore gives exactly the
  bundled zone (`zone_source_tests.rs` checks every zone against
  `jiff-tzdb`; the five zones with negative daylight saving time differ
  only in their `is_dst` flags, since `jiff-tzdb` compiles the rearguard
  format).
- **Interning.** Zone data is stored once per distinct zone for the life
  of the process. That is what keeps `TimeZone` a `Copy` value that draws
  and queries pass around, and what lets the queries return abbreviations
  and primary names as static strings, as they do for the bundled table.
  Memory grows with the number of distinct zones ever added, not with the
  number of sources or uploads.
- **The probe.** With `next_transition` the engine walks the transitions
  one by one and stops after `last_year`; without it, it samples once a day
  and bisects each change to the second. Before `first_year` the type at
  its start holds. A probe that takes more than 100,000 steps (transitions
  `next_transition` reports, whether or not the type changes there, or
  changes the sampling finds), or a `next_transition` that does not move
  forward, is an argument error.
  `HEGEL_ZONE_AFTER_INFER` is also accepted for structured data, so a
  frontend that probes on its own (the Wasm build) gets the same
  inference.
- **Inference.** Neither of the last two years changing anything gives a
  fixed rule. Otherwise each must change exactly twice, alternating
  between the same two types; standard time is the one whose `is_dst` is
  clear or, when the flags agree, the smaller offset. Each transition date
  is matched against the rule days naming it (last or n-th weekday of the
  month, Julian day, zero-based day, the day before or after for times
  past midnight), and of those naming both years' dates, the one that also
  names the most consecutive earlier years' (up to 28) wins. Anything else
  holds the last type. Probing every bundled zone whose rule is in force
  by 2029 over 1990 to 2030 gives back its rule exactly.

### 3.2 Names

- **Loading.** A source's name is a zone, a link, or declared: known, with
  data to come. A lookup of a declared name, or of a name the source does
  not know at all, asks the loader, once per name; a name the loader does
  not know is remembered as missing until declared again. A declared name
  the loader supplies nothing for, a link to a name nothing supplies, a
  chain of more than 16 links (counting links only, however many of them
  the loader had to supply), and a loader's failure are argument errors,
  the last with the loader's message. Without a loader, a declared name can
  be drawn but not evaluated: a zoned draw or query of it is an argument
  error.
- **Laziness.** A spec draws its names from the catalogue's entries, so a
  name the catalogue has never loads to be drawn: with a loader, only the
  zones a zoned draw lands on load. A name the catalogue lacks loads when
  the spec is first validated or drawn from, since its tags come from its
  data.
- **Without callbacks.** A host that cannot be called back (the Wasm
  build) declares the names, draws the name with `hegel_generate_timezone`,
  adds that zone's data, then draws the value in that zone with
  `hegel_generate_zoned_datetime_for_zone`. The second call records no zone
  node of its own, so the two calls record what
  `hegel_generate_zoned_datetime` records in one, and a failure found one
  way replays the other way. The function also serves a frontend whose
  user supplies the zone. It takes a name the source knows or
  `Etc/Unknown`, and never treats another name as opaque: for an opaque
  name the first step drew, the frontend passes `Etc/Unknown`, which
  records the nodes an opaque zone's value does, and reports the value
  under the name it drew.
- **The catalogue.** A name the catalogue has keeps its flags, tags and
  rank whatever data the source has for it. A name it lacks has the rank
  after every catalogued name and the tags the classifier gives its data
  and spelling (a link's are its target's data tags plus `link`); one not
  loaded yet has its spelling's tags only. A spelling never makes such a
  name `legacy`: its area says nothing about a name IANA lacks, so
  `Mars/Base` is drawn by default. The spec's ladder is the
  catalogued names by rank, then the rest by name, then opaque names, then
  `Etc/Unknown`. `classify_tests.rs` checks the classifier against every
  bundled name's tags.
- **Listing.** `hegel_zone_source_name_count` and `hegel_zone_source_name`
  read the names a source knows, the bundled one included, in ascending
  byte order: its zones, links and declared names, but not a name its
  loader was asked for and did not have. A host library that can load a
  zone by name but cannot list its names (Go's `time`) lists the bundled
  source's and declares the ones it loads.
- **The spec.** Without an explicit list a spec draws the source's names
  the link and legacy flags allow. An explicit list must name names the
  source knows, unless opaque names are accepted. Neither is checked by a
  setter, which reports only a bad argument of its own, so the setters can
  be called in any order: `hegel_timezone_spec_validate` and the spec's
  draws check the spec as it then stands, and report a name the source
  lacks or a spec with nothing to draw as an argument error. A spec that
  fails can be fixed with further setters and used again.

### 3.3 The default and versions

- **Precedence.** A spec reads the process default when
  `hegel_timezone_spec_new` creates it, and again on
  `hegel_timezone_spec_set_source(spec, NULL)`; a spec created before the
  default changes keeps the source it read, as a string generator keeps
  the Unicode version it was built with. The queries without a source, and
  `hegel_generate_zoned_datetime_for_zone` with a NULL source, read the
  default at each call.
- **Unchanged without a source.** With no source set, the bundled source
  takes the path the engine took before zone sources, so draws, choice
  sequences and query results are unchanged.
- **Versions.** `hegel_tzdb_version` stays the bundled release. A custom
  source's version is what `hegel_zone_source_set_version` recorded, or
  empty; nothing in failure output reports it (§6).
- **Callbacks.** A callback gets a fresh context and returns an
  `int32_t`: 0 for success, anything else for failure (not a
  `hegel_result_t`, so a host that maps its own error codes or exceptions
  onto the return value cannot hand the engine an enum value it does not
  have). The failure's message, set with `hegel_context_set_error` or by a
  failed libhegel call, becomes the argument error the draw, query or
  upload reports; without one the error names the callback and the value
  it returned. Nothing a callback does can panic the engine.
- **The loader's lifetime.** `hegel_zone_source_set_loader` takes a
  `release` hook next to the loader's `user_data`. The engine calls it
  exactly once, with that `user_data`, when it lets go of the loader: when
  another loader replaces it, when a NULL loader removes it, when the
  source's last holder (a handle, a spec or the process default) goes, or
  before returning when the call that offered it fails; never while a call
  of the loader is in flight, so possibly later than that event and on
  another thread. A frontend allocates the loader's state, hands it over,
  and frees it in the hook, so nothing has to outlive the source by
  convention. A NULL `release` means the state needs no freeing, and a NULL
  loader's `user_data` and `release` are ignored. Oracle callbacks need no
  hook: they are only called during `hegel_zone_source_add_oracle`.
- **Scalar draws.** The UTC-offset and timestamp draws, which target the
  transitions of every bundled zone, keep the bundled table: they draw no
  zone, so there is no source to read.
- **Replay.** Choice sequences stay valid across hosts: every recorded
  sequence replays to some value on any data. Zones are recorded by name,
  and an untargeted wall time as civil fields, so those replay to the same
  name and wall time, with only the classification differing on a host
  whose data differs. A targeted draw does not: a draw aimed at a
  transition records a position among the zone's transitions, and the
  instant fallback of `datetime.md` §6.2 one among the zone's values in
  the bounds, so on different data the same choices decode to a different
  wall time (still within the bounds, still a value the generator can
  produce). A stored failure that hinges on a particular
  transition is therefore tied to the data it was found on, the same
  trade the Unicode version makes.

## 4. Per frontend

Every frontend exposes a zone-source abstraction in its own idiom, so that
an integration with any date/time library can supply one, not just the
built-ins below. Each one says which database it draws against, and
generators built on a library default to that library's source.

| Frontend / library | Shape | Source of truth | Notes |
|---|---|---|---|
| Python, `zoneinfo` | TZif, lazy | the same lookup `zoneinfo` does: `zoneinfo.TZPATH` directories first, then the `tzdata` package | Version from `tzdata.IANA_VERSION`, or `tzdata.zi`/`+VERSION` in the TZPATH directory when present. The process default. |
| Java, `java.time` | structured, lazy | `ZoneRulesProvider`, via `ZoneId.of(name).getRules()` | `ZoneOffsetTransitionRule` (month, day indicator, day of week, local time, UTC/STANDARD/WALL time definition) maps onto a footer DST rule. Version from `ZoneRulesProvider.getVersions(id)`. The process default. |
| Rust core | bundled | engine | No host database: the process default is the bundled one. |
| Rust, jiff extra | oracle, lazy | `jiff::tz::db()`: the names it lists, and each zone's offsets and transitions (`TimeZone::following`) probed through jiff itself | Whichever database jiff reads (the system's zoneinfo or its bundled copy), so the classification is jiff's own. jiff does not report a release, so the version is empty. |
| Rust, chrono-tz extra | oracle, lazy | `chrono_tz::Tz` offsets | chrono-tz compiles a fixed release (`IANA_TZDB_VERSION`) into private tables with a final year, and probing reproduces exactly what it computes. Alternative: when its version equals the engine's, use the bundled data and skip probing. |
| Rust, `time` extra | none | — | `time` has offsets only. |
| C++ core | bundled | engine | The process default. |
| C++, `std::chrono` (future) | oracle | `time_zone::get_info` | Once the toolchains support it. |
| C++, Boost.DateTime | structured | `boost::local_time::time_zone_ptr` (`tz_database` CSV or a POSIX string) | Footer-only zones. A Boost zone has the name its database gives it, and names the catalogue lacks get computed tags. |
| TypeScript, Temporal present | oracle (next transition) | `Temporal.ZonedDateTime#getTimeZoneTransition` | The process default where `Temporal` exists. Exact transitions within the probe range; footer inferred beyond it. |
| TypeScript, `Intl` only | oracle (offset at t) | `Intl.DateTimeFormat` with `timeZoneName: "longOffset"` | The process default otherwise. Slower probing; lazy, per drawn zone. Keep today's reject on disagreement as a safety net for anything beyond the probe range. |
| Go, `time` | oracle (next transition), lazy | `time.LoadLocation`: each zone's offsets and transitions (`Time.ZoneBounds`) probed through the loaded `*time.Location` | The frontend's default. Go cannot list its zone names, so the source declares the names of the bundled source (`hegel_zone_source_name`) that `time.LoadLocation` loads. The frontend keeps the default itself and attaches the source to each spec. `ZoneSourceFromLocations` builds a source the same way from given locations. |

Wherever a source is exact (TZif, `java.time`, Boost), the
reject-on-disagreement checks in Java, Python and TypeScript become dead
code and can go. Where a source is an oracle, they stay, for instants
beyond the probe range.

## 5. Phasing

All five phases are delivered, and the OCaml and Go frontends, which the
plan below does not list, have zone sources too. Each frontend's own
documentation is the reference for what it built.

1. **Engine:** zone sources, a TZif parser, structured upload, the loader
   callback, the process default and per-spec attachment, source-aware
   queries, and the tag classifier. The catalogue-only zone draw already
   exists.
2. **Python and Java:** the two exact host databases. These remove most of
   today's rejections.
3. **Rust jiff and chrono-tz extras**, and a public `ZoneSource` trait for
   third-party integrations.
4. **C++:** the `ZoneSource` interface plus a Boost.DateTime adapter,
   optional and behind a CMake option, so the core keeps no Boost
   dependency.
5. **TypeScript:** Temporal first, then the `Intl` probe.

## 6. Decisions

Settled by the maintainer on 2026-10-23. Zone sources ship in the same
release as the date and time API (PR #16), since they are part of getting
that API right.

- **Default and override.** Every frontend sets a process-wide default
  source: the standard library's database where the language has one, the
  bundled database where it does not. A draw can override it, and a
  library integration's generators always do.
- **Unknown names.** The tag classifier moves into the engine, so a name
  the catalogue lacks gets real tags (and the last rank).
  `generate_tzdata.py` keeps building the catalogue.
- **Version reporting.** Failure output does not name the source's
  version. The generator implies which library's database a value came
  from. Which install of that data a host has (system zone files or the
  `tzdata` package for Python, the JDK build's data for Java, system or
  bundled data for jiff) can still differ between machines, but a failure
  that hinges on that is rare and needs debugging anyway. Revisit if it
  turns out to matter.
- **After the probe range.** An oracle source can only be probed over a
  finite range of years, but the engine draws years out to ±999,999, so
  it needs a rule beyond the last probed year. Whoever builds the source
  picks one of two rules. Holding the last offset is exact for chrono-tz,
  which itself holds the last offset after its table ends. Inferring a
  recurring yearly rule (a footer) from the last two probed years'
  transitions is closer for `Temporal` and `Intl`, which apply the IANA
  rule forever. The TypeScript frontend keeps its reject-on-disagreement
  check as a net for anything inference gets wrong.
