RELEASE_TYPE: minor

This release adds the `Hegel.Temporal` date and time generators and deprecates the string `dates`, `times` and `datetimes` generators they supersede.

`Hegel.Temporal` generates dates, times, naive datetimes, instants, UTC offsets, time zone names, zoned datetimes, signed durations and calendar spans:

| Generator | Value | Options |
|---|---|---|
| `Temporal.dates` | `Temporal.Date.t` | `~min_value`, `~max_value`, `~weekdays`, `~months` |
| `Temporal.times` | `Temporal.Time.t` | `~min_value`, `~max_value`, `~precision` |
| `Temporal.datetimes` | `Temporal.Datetime.t` | `~min_value`, `~max_value`, `~precision` |
| `Temporal.instants` | `Temporal.Instant.t` | `~min_value`, `~max_value`, `~precision` |
| `Temporal.offsets` | `Temporal.Offset.t` | `~min_value`, `~max_value` (seconds east of UTC), `~granularity`, `~allow_unknown` |
| `Temporal.zones` | `string` | `~names`, `~allow_links`, `~allow_legacy`, `~allow_unknown`, `~allow_opaque`, `~source` |
| `Temporal.zoned_datetimes` | `Temporal.Zoned_datetime.t` | `~zones`, `~source`, `~min_value` and `~max_value` or `~min_instant` and `~max_instant`, `~precision`, `~allow_imaginary`, `~ambiguous` |
| `Temporal.durations` | `Temporal.Duration.t` | `~min_value`, `~max_value`, `~precision` |
| `Temporal.spans` | `Temporal.Span.t` | inclusive bounds per unit, such as `~days:(0, 30)`, and `~sign` |

Each value is a plain record or variant in its own module, with `to_string`, `pp`, `sexp_of_t`, `equal`, `compare` and a default generator `hegel_generator`, so `[@@deriving hegel_generator]` works on a type with a `Temporal` field. The text is the one every Hegel frontend prints, such as `2024-03-10T02:30:00-04:00[America/New_York][gap]`, and failure output shows it without s-expression quoting. The enumerated options are polymorphic variants: `` ~precision:`Seconds ``, `` ~weekdays:[ `Saturday; `Sunday ] ``, `` ~ambiguous:`Later ``, `` ~sign:`Mixed ``.

```ocaml
let new_york =
  Temporal.zoned_datetimes
    ~zones:(Temporal.zones ~names:[ "America/New_York" ] ())
    ~precision:`Seconds
    ()

let%hegel_test skipped_hours tc =
  let z = draw tc new_york in
  match z.kind with
  | Gap -> assert (not (Temporal.Offset.equal z.offset_before z.offset_after))
  | Unique | Fold -> ()
```

`Temporal.zoned_datetimes` draws a zone and a wall time in it, and targets the zone's transitions so that skipped (`Gap`) and repeated (`Fold`) wall times turn up in short runs. Zones and wall times are drawn against the IANA database bundled in the engine (`Temporal.tzdb_version ()`), or against a `Temporal.Zone_source.t` filled from TZif files, zone data, an offset oracle or a lazy loader. Choose the source for one generator with `~source`, or for the process with `Temporal.set_default_zone_source`, which returns the previous default. A generator reads the default when it is built, so set it first. When `~zones` is anything but a `Temporal.zones` generator (a `filter` or `map` over one, or `sampled_from`), `zoned_datetimes` looks its names up in `~source`, and a name that source lacks raises `Usage_error`. `Temporal.resolve`, `Temporal.offset_at` and `Temporal.info` query a source directly.

The string `dates`, `times` and `datetimes` generators and the `make_dates`, `make_times` and `make_datetimes` builders are deprecated, and the `date` and `time` types are now aliases of `Temporal.Date.t` and `Temporal.Time.t`, documented as deprecated. The generators still produce exactly the values they did, but now raise a deprecation alert, which dune's default development profile treats as an error. Draw the `Temporal` records instead, and map them to strings or your own type:

```ocaml
(* before *)
let d = draw tc (dates ~min_date:{ year = 2024; month = 1; day = 1 } ()) in
(* after *)
let d = draw tc (Temporal.dates ~min_value:{ year = 2024; month = 1; day = 1 } ()) in
```

`Temporal.dates` defaults to the engine's whole range, years -999999 to 999999, where `dates` defaulted to years 1 to 9999. Until you migrate, `[@alert "-deprecated"]` silences the alert.

`Hegel_jane.dates`, `ofdays` and `times` keep their signatures and ranges. `Hegel_jane.times` now draws an instant rather than a date and a time of day, so a failure of a test using it that an earlier version stored may not replay.
