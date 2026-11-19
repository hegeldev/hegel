RELEASE_TYPE: minor

This release adds `Hegel.Temporal`, with typed generators for dates, times, naive datetimes, instants, UTC offsets, time zone names, zoned datetimes, signed durations and calendar spans.

Zoned datetimes include skipped (`Gap`) and repeated (`Fold`) wall times:

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

Time zones use Hegel's bundled IANA database by default. To use your own data, create a `Temporal.Zone_source.t` from TZif files, structured zone data, lookup callbacks or a lazy loader. Pass `~source` to a generator, or call `Temporal.set_default_zone_source` before creating generators. `Temporal.resolve`, `Temporal.offset_at` and `Temporal.info` query the source directly.

The string `dates`, `times` and `datetimes` generators, the `make_dates`, `make_times` and `make_datetimes` builders, and the `date` and `time` types are deprecated. The types are now aliases of `Temporal.Date.t` and `Temporal.Time.t`.

Use the `Temporal` records and map them to strings or your own types as needed:

```ocaml
(* before *)
let d = draw tc (dates ~min_date:{ year = 2024; month = 1; day = 1 } ()) in
(* after *)
let d = draw tc (Temporal.dates ~min_value:{ year = 2024; month = 1; day = 1 } ()) in
```

`Temporal.dates` defaults to years -999999 to 999999, while the old `dates` generator defaults to years 1 to 9999.
