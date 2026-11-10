open Hegel.Temporal
module Z = Zone_source

let settings ?(test_cases = 100) () = Hegel.Settings.create ~test_cases ~seed:0 ()
let run ?test_cases f = Hegel.run_hegel_test ~settings:(settings ?test_cases ()) f
let check_string = Alcotest.(check string)

let expect_usage_error_now substring f =
  match f () with
  | _ -> Alcotest.fail "expected Usage_error"
  | exception Hegel.Usage_error msg ->
    Alcotest.(check bool)
      (Printf.sprintf "diagnostic %S contains %S" msg substring)
      true
      (Test_helpers.contains_substring msg substring)
;;

let date year month day : Date.t = { year; month; day }

let time ?(nanosecond = 0) hour minute second : Time.t =
  { hour; minute; second; nanosecond }
;;

let civil y mo d h mi : Datetime.t = { date = date y mo d; time = time h mi 0 }
let instant ?(nanosecond = 0) second : Instant.t = { second; nanosecond }

let core_date ({ year; month; day } : Date.t) =
  Core.Date.create_exn ~y:year ~m:(Core.Month.of_int_exn month) ~d:day
;;

let epoch = Core.Date.create_exn ~y:1970 ~m:Jan ~d:1

let utc_seconds ({ date; time } : Datetime.t) =
  (Core.Date.diff (core_date date) epoch * 86_400)
  + (time.hour * 3600)
  + (time.minute * 60)
  + time.second
;;

module type Value = sig
  type t

  val to_string : t -> string
  val pp : Format.formatter -> t -> unit
  val sexp_of_t : t -> Sexplib0.Sexp.t
  val equal : t -> t -> bool
  val compare : t -> t -> int
  val hegel_generator : (t, Hegel.printable) Hegel.generator
end

let check_text (type a) (module V : Value with type t = a) name expected (value : a) =
  check_string name expected (V.to_string value);
  check_string (name ^ " (pp)") expected (Format.asprintf "%a" V.pp value);
  Alcotest.(check bool)
    (name ^ " (sexp)")
    true
    (Sexplib0.Sexp.equal (V.sexp_of_t value) (Atom expected))
;;

let check_order (type a) (module V : Value with type t = a) name (low : a) (high : a) =
  Alcotest.(check bool) (name ^ " equals itself") true (V.equal low low);
  Alcotest.(check bool) (name ^ " differs") false (V.equal low high);
  Alcotest.(check int) (name ^ " compares equal") 0 (V.compare low low);
  Alcotest.(check bool) (name ^ " below") true (V.compare low high < 0);
  Alcotest.(check bool) (name ^ " above") true (V.compare high low > 0)
;;

let check_default_generator (type a) (module V : Value with type t = a) =
  run ~test_cases:20 (fun tc ->
    let value = Hegel.draw tc V.hegel_generator in
    Alcotest.(check bool) "drawn value equals itself" true (V.equal value value))
;;

let test_date_text () =
  let check = check_text (module Date) in
  check "plain" "2024-03-10" (date 2024 3 10);
  check "padded" "0007-01-02" (date 7 1 2);
  check "large" "+010000-12-31" (date 10_000 12 31);
  check "negative" "-000001-06-15" (date (-1) 6 15);
  check "first" "-999999-01-01" Date.min_value;
  check "last" "+999999-12-31" Date.max_value;
  check "fields out of range" "+2147483647-255-255" (date (1 lsl 40) 300 (-1))
;;

let test_time_text () =
  let check = check_text (module Time) in
  check "whole" "02:30:00" (time 2 30 0);
  check "half" "23:59:59.5" (time ~nanosecond:500_000_000 23 59 59);
  check "nano" "00:00:00.000000001" (time ~nanosecond:1 0 0 0);
  check "first" "00:00:00" Time.min_value;
  check "last" "23:59:59.999999999" Time.max_value;
  check_text
    (module Datetime)
    "datetime"
    "2024-03-10T02:30:00.12"
    { date = date 2024 3 10; time = time ~nanosecond:120_000_000 2 30 0 };
  check_text
    (module Datetime)
    "first datetime"
    "-999999-01-01T00:00:00"
    Datetime.min_value;
  check_text
    (module Datetime)
    "last datetime"
    "+999999-12-31T23:59:59.999999999"
    Datetime.max_value
;;

let test_instant_text () =
  let check = check_text (module Instant) in
  check "epoch" "1970-01-01T00:00:00Z" (instant 0);
  check "y2k" "2000-01-01T00:00:00Z" (instant 946_684_800);
  check "before epoch" "1969-12-31T23:59:58.5Z" (instant ~nanosecond:500_000_000 (-2));
  check "leap day" "2024-02-29T12:00:00Z" (instant 1_709_208_000);
  check "min" "-999999-01-01T00:00:00Z" Instant.min_value;
  check "max" "+999999-12-31T23:59:59.999999999Z" Instant.max_value;
  check "year zero" "0000-03-01T00:00:00Z" (instant (-62_162_035_200));
  check "before year zero" "-000001-12-31T00:00:00Z" (instant (-62_167_305_600))
;;

let test_instant_text_matches_core () =
  run (fun tc ->
    let i =
      Hegel.draw
        tc
        (instants
           ~min_value:(instant (-62_135_596_800))
           ~max_value:(instant 253_402_300_799)
           ~precision:`Seconds
           ())
    in
    let days =
      Int.div (i.second - (((i.second mod 86_400) + 86_400) mod 86_400)) 86_400
    in
    let expected = Core.Date.to_string (Core.Date.add_days epoch days) in
    check_string "date part" expected (String.sub (Instant.to_string i) 0 10))
;;

let test_offset_text () =
  let check = check_text (module Offset) in
  check "unknown" "-00:00" Unknown;
  check "zero" "+00:00" (Known 0);
  check "india" "+05:30" (Known 19_800);
  check "amsterdam" "+00:19:32" (Known 1172);
  check "monrovia" "-00:44:30" (Known (-2670));
  check "max" "+25:59:59" (Known Offset.max_seconds)
;;

let zoned
      ?(zone = "America/New_York")
      ?(kind = Zoned_datetime.Unique)
      ?(fold = false)
      offset
  : Zoned_datetime.t
  =
  { zone
  ; civil = civil 2024 3 10 1 30
  ; kind
  ; offset_before = offset
  ; offset_after = offset
  ; fold
  ; offset
  ; instant = instant 0
  }
;;

let test_zoned_text () =
  let check = check_text (module Zoned_datetime) in
  check "unique" "2024-03-10T01:30:00-05:00[America/New_York]" (zoned (Known (-18_000)));
  check
    "gap"
    "2024-03-10T01:30:00-05:00[America/New_York][gap]"
    (zoned ~kind:Gap (Known (-18_000)));
  check
    "fold 0"
    "2024-03-10T01:30:00-04:00[America/New_York][fold=0]"
    (zoned ~kind:Fold (Known (-14_400)));
  check
    "fold 1"
    "2024-03-10T01:30:00-05:00[America/New_York][fold=1]"
    (zoned ~kind:Fold ~fold:true (Known (-18_000)));
  check
    "unknown zone"
    "2024-03-10T01:30:00Z[Etc/Unknown]"
    (zoned ~zone:unknown_zone Unknown);
  check
    "opaque zone"
    "2024-03-10T01:30:00-00:00[Mars/Olympus]"
    (zoned ~zone:"Mars/Olympus" Unknown)
;;

let test_duration_text () =
  let check name expected seconds nanoseconds =
    check_text (module Duration) name expected { seconds; nanoseconds }
  in
  check "zero" "PT0S" 0 0;
  check "mixed" "PT1H30M5.5S" 5405 500_000_000;
  check "hours" "PT2H" 7200 0;
  check "minutes" "PT1M" 60 0;
  check "hours seconds" "PT1H1S" 3601 0;
  check "negative" "-PT1.5S" (-1) (-500_000_000);
  check "negative fraction" "-PT0.000000001S" 0 (-1);
  check "days" "PT48H" 172_800 0
;;

let span : Span.t =
  { years = 0
  ; months = 0
  ; weeks = 0
  ; days = 0
  ; hours = 0
  ; minutes = 0
  ; seconds = 0
  ; milliseconds = 0
  ; microseconds = 0
  ; nanoseconds = 0
  }
;;

let test_span_text () =
  let check = check_text (module Span) in
  check "zero" "0s" span;
  check
    "all"
    "1y 2mo 3w 4d 5h 6m 7s 8ms 9\xc2\xb5s 10ns"
    { years = 1
    ; months = 2
    ; weeks = 3
    ; days = 4
    ; hours = 5
    ; minutes = 6
    ; seconds = 7
    ; milliseconds = 8
    ; microseconds = 9
    ; nanoseconds = 10
    };
  check "negative" "-3d 4h" { span with days = -3; hours = -4 };
  check "mixed" "-3d +4h" { span with days = -3; hours = 4 };
  check "mixed, positive first" "+1y -2d" { span with years = 1; days = -2 }
;;

let test_order () =
  check_order (module Date) "date" (date 2024 3 10) (date 2024 3 11);
  check_order (module Time) "time" (time 2 30 0) (time ~nanosecond:1 2 30 0);
  check_order (module Datetime) "datetime" (civil 2024 3 10 23 59) (civil 2024 3 11 0 0);
  check_order (module Instant) "instant" (instant ~nanosecond:5 (-2)) (instant (-1));
  check_order (module Offset) "offset" (Known (-3600)) (Known 3600);
  check_order (module Offset) "unknown offset" Unknown (Known (-3600));
  check_order
    (module Duration)
    "duration"
    { seconds = -1; nanoseconds = -5 }
    { seconds = 0; nanoseconds = 1 };
  check_order (module Span) "span" span { span with nanoseconds = 1 };
  check_order
    (module Zoned_datetime)
    "zoned"
    (zoned (Known (-18_000)))
    (zoned ~zone:"Asia/Kolkata" (Known 19_800))
;;

let test_default_generators () =
  check_default_generator (module Date);
  check_default_generator (module Time);
  check_default_generator (module Datetime);
  check_default_generator (module Instant);
  check_default_generator (module Offset);
  check_default_generator (module Zoned_datetime);
  check_default_generator (module Duration);
  check_default_generator (module Span)
;;

let with_captured_failure f =
  Test_stateful.with_captured_stderr (fun () ->
    match
      Hegel.run_hegel_test
        ~settings:
          { (Hegel.Settings.create ~test_cases:5 ~seed:0 ()) with
            database = Hegel.Settings.Disabled
          }
        (fun tc ->
           f tc;
           failwith "stop here")
    with
    | () -> Alcotest.fail "expected the test to fail"
    | exception Failure _ -> ())
;;

let test_failure_output () =
  let day = date 2024 3 10 in
  let gap = civil 2024 3 10 2 30 in
  let output =
    with_captured_failure (fun tc ->
      ignore
        (Hegel.draw tc (spans ~years:(1, 1) ~months:(2, 2) ~microseconds:(9, 9) ())
         : Span.t);
      ignore
        (Hegel.draw
           tc
           (Hegel.lists
              (dates ~min_value:day ~max_value:day ())
              ~min_size:2
              ~max_size:2
              ())
         : Date.t list);
      ignore
        (Hegel.draw
           tc
           (zoned_datetimes
              ~zones:(zones ~names:[ "America/New_York" ] ())
              ~min_value:gap
              ~max_value:gap
              ())
         : Zoned_datetime.t);
      ignore (Hegel.draw tc (zones ~names:[ "Asia/Calcutta" ] ()) : string))
  in
  List.iter
    (fun expected ->
       Alcotest.(check bool)
         (Printf.sprintf "%S is in %S" expected output)
         true
         (Test_helpers.contains_substring output expected))
    [ "= 1y 2mo 9\xc2\xb5s"
    ; "= (2024-03-10 2024-03-10)"
    ; "= 2024-03-10T02:30:00-04:00[America/New_York][gap]"
    ; "= Asia/Calcutta"
    ]
;;

let weekdays : (weekday * Core.Day_of_week.t) list =
  [ `Monday, Mon
  ; `Tuesday, Tue
  ; `Wednesday, Wed
  ; `Thursday, Thu
  ; `Friday, Fri
  ; `Saturday, Sat
  ; `Sunday, Sun
  ]
;;

let months : month list =
  [ `January
  ; `February
  ; `March
  ; `April
  ; `May
  ; `June
  ; `July
  ; `August
  ; `September
  ; `October
  ; `November
  ; `December
  ]
;;

let test_dates () =
  let min_value = date 1 1 1 in
  let max_value = date 9999 12 31 in
  List.iter
    (fun (weekday, expected) ->
       run ~test_cases:10 (fun tc ->
         let d = Hegel.draw tc (dates ~min_value ~max_value ~weekdays:[ weekday ] ()) in
         Alcotest.(check bool)
           "weekday"
           true
           (Core.Day_of_week.equal expected (Core.Date.day_of_week (core_date d)))))
    weekdays;
  List.iteri
    (fun index month ->
       run ~test_cases:10 (fun tc ->
         let d = Hegel.draw tc (dates ~min_value ~max_value ~months:[ month ] ()) in
         Alcotest.(check int) "month" (index + 1) d.month))
    months;
  run (fun tc ->
    let d =
      Hegel.draw
        tc
        (dates
           ~min_value
           ~max_value
           ~weekdays:[ `Saturday; `Sunday ]
           ~months:[ `February ]
           ())
    in
    Alcotest.(check int) "month" 2 d.month;
    Alcotest.(check bool)
      "weekend"
      true
      (Core.Day_of_week.is_sun_or_sat (Core.Date.day_of_week (core_date d))));
  run (fun tc ->
    let lo = Hegel.draw tc (dates ~min_value ~max_value ()) in
    let hi = Hegel.draw tc (dates ~min_value:lo ~max_value ()) in
    let d = Hegel.draw tc (dates ~min_value:lo ~max_value:hi ()) in
    assert (Date.compare lo d <= 0 && Date.compare d hi <= 0);
    ignore (core_date d : Core.Date.t));
  let saw_extended = ref false in
  run (fun tc ->
    let d = Hegel.draw tc (dates ()) in
    assert (Date.compare Date.min_value d <= 0 && Date.compare d Date.max_value <= 0);
    if d.year < 0 || d.year > 9999 then saw_extended := true);
  Alcotest.(check bool) "default range is the engine's" true !saw_extended
;;

let test_dates_invalid () =
  Test_helpers.expect_usage_error (dates ~weekdays:[] ()) "weekday";
  Test_helpers.expect_usage_error (dates ~months:[] ()) "month";
  Test_helpers.expect_usage_error
    (dates ~min_value:(date 2024 1 2) ~max_value:(date 2024 1 1) ())
    "min_value <= max_value"
;;

let precisions : (precision * int) list =
  [ `Nanoseconds, 1
  ; `Microseconds, 1_000
  ; `Milliseconds, 1_000_000
  ; `Seconds, 1_000_000_000
  ; `Minutes, 60_000_000_000
  ; `Hours, 3_600_000_000_000
  ]
;;

let test_times () =
  List.iter
    (fun (precision, unit) ->
       run ~test_cases:30 (fun tc ->
         let t = Hegel.draw tc (times ~precision ()) in
         let ns =
           (((((t.hour * 60) + t.minute) * 60) + t.second) * 1_000_000_000) + t.nanosecond
         in
         Alcotest.(check int) "precision" 0 (ns mod unit)))
    precisions;
  run (fun tc ->
    let min_value = time 9 0 0 in
    let max_value = time 17 0 0 in
    let t = Hegel.draw tc (times ~min_value ~max_value ()) in
    assert (Time.compare min_value t <= 0 && Time.compare t max_value <= 0))
;;

let test_datetimes () =
  let min_value = civil 2024 3 10 12 0 in
  let max_value = civil 2024 3 11 12 0 in
  run (fun tc ->
    let dt = Hegel.draw tc (datetimes ~min_value ~max_value ~precision:`Minutes ()) in
    assert (Datetime.compare min_value dt <= 0 && Datetime.compare dt max_value <= 0);
    Alcotest.(check int) "seconds" 0 dt.time.second);
  run ~test_cases:20 (fun tc ->
    let dt = Hegel.draw tc (datetimes ()) in
    assert (Date.compare Date.min_value dt.date <= 0))
;;

let test_instants () =
  let min_value = instant (-86_400) in
  let max_value = instant 86_400 in
  run (fun tc ->
    let i = Hegel.draw tc (instants ~min_value ~max_value ~precision:`Milliseconds ()) in
    assert (Instant.compare min_value i <= 0 && Instant.compare i max_value <= 0);
    Alcotest.(check int) "millis" 0 (i.nanosecond mod 1_000_000));
  run ~test_cases:20 (fun tc ->
    let i = Hegel.draw tc (instants ()) in
    assert (
      Instant.compare Instant.min_value i <= 0 && Instant.compare i Instant.max_value <= 0))
;;

let granularities : (granularity * int) list =
  [ `Hours, 3600; `Quarter_hours, 900; `Minutes, 60; `Seconds, 1 ]
;;

let test_offsets () =
  List.iter
    (fun (granularity, unit) ->
       run ~test_cases:30 (fun tc ->
         match Hegel.draw tc (offsets ~granularity ()) with
         | Known s ->
           Alcotest.(check int) "granularity" 0 (s mod unit);
           assert (abs s <= Offset.max_seconds)
         | Unknown -> Alcotest.fail "unknown offset without ~allow_unknown"))
    granularities;
  let saw_unknown = ref false in
  run (fun tc ->
    match
      Hegel.draw tc (offsets ~min_value:(-3600) ~max_value:3600 ~allow_unknown:true ())
    with
    | Unknown -> saw_unknown := true
    | Known s -> assert (abs s <= 3600));
  Alcotest.(check bool) "unknown drawn" true !saw_unknown;
  Test_helpers.expect_usage_error
    (offsets ~min_value:3600 ~max_value:(-3600) ())
    "min_seconds <= max_seconds"
;;

let test_durations () =
  let min_value : Duration.t = { seconds = -10; nanoseconds = 0 } in
  let max_value : Duration.t = { seconds = 10; nanoseconds = 0 } in
  run (fun tc ->
    let d = Hegel.draw tc (durations ~min_value ~max_value ~precision:`Microseconds ()) in
    assert (Duration.compare min_value d <= 0 && Duration.compare d max_value <= 0);
    assert (
      (d.seconds <= 0 && d.nanoseconds <= 0) || (d.seconds >= 0 && d.nanoseconds >= 0));
    Alcotest.(check int) "micros" 0 (d.nanoseconds mod 1000));
  run ~test_cases:20 (fun tc ->
    let d = Hegel.draw tc (durations ()) in
    assert (abs d.nanoseconds <= 999_999_999))
;;

let span_values (s : Span.t) =
  [ s.years
  ; s.months
  ; s.weeks
  ; s.days
  ; s.hours
  ; s.minutes
  ; s.seconds
  ; s.milliseconds
  ; s.microseconds
  ; s.nanoseconds
  ]
;;

let test_spans () =
  run (fun tc ->
    let s = Hegel.draw tc (spans ~days:(0, 30) ~hours:(0, 23) ()) in
    assert (s.days >= 0 && s.days <= 30 && s.hours >= 0 && s.hours <= 23);
    Alcotest.(check (list int))
      "other units"
      [ 0; 0; 0; 0; 0; 0; 0; 0 ]
      (List.filteri (fun i _ -> i <> 3 && i <> 4) (span_values s)));
  run (fun tc ->
    let s = Hegel.draw tc (spans ~sign:`Non_negative ()) in
    assert (List.for_all (fun v -> v >= 0) (span_values s)));
  run (fun tc ->
    let s = Hegel.draw tc (spans ~sign:`Uniform ()) in
    let values = span_values s in
    assert (List.for_all (fun v -> v >= 0) values || List.for_all (fun v -> v <= 0) values));
  let saw_mixed = ref false in
  run (fun tc ->
    let values =
      span_values
        (Hegel.draw
           tc
           (spans
              ~years:(-5, 5)
              ~months:(-5, 5)
              ~weeks:(-5, 5)
              ~days:(-5, 5)
              ~hours:(-5, 5)
              ~minutes:(-5, 5)
              ~seconds:(-5, 5)
              ~milliseconds:(-5, 5)
              ~microseconds:(-5, 5)
              ~nanoseconds:(-5, 5)
              ~sign:`Mixed
              ()))
    in
    if List.exists (fun v -> v > 0) values && List.exists (fun v -> v < 0) values
    then saw_mixed := true);
  Alcotest.(check bool) "mixed signs drawn" true !saw_mixed;
  run ~test_cases:10 (fun tc -> ignore (Hegel.draw tc (spans ~sign:`Mixed ()) : Span.t));
  Test_helpers.expect_usage_error (spans ~days:(5, 1) ()) "min_value";
  Test_helpers.expect_usage_error (spans ~days:(-1, 1) ~sign:`Non_negative ()) ""
;;

let aaa : Z.zone_type = { offset_seconds = 0; is_dst = false; abbreviation = "AAA" }
let bbb : Z.zone_type = { offset_seconds = 3600; is_dst = true; abbreviation = "BBB" }
let wide = 1 lsl 32

let test_out_of_range_fields_are_rejected () =
  Test_helpers.expect_usage_error (dates ~min_value:(date 2024 1 (wide + 1)) ()) "";
  Test_helpers.expect_usage_error (dates ~min_value:(date 2024 257 1) ()) "";
  Test_helpers.expect_usage_error (dates ~max_value:(date (wide + 2024) 1 1) ()) "";
  Test_helpers.expect_usage_error (dates ~max_value:(date (-wide + 2024) 1 1) ()) "";
  Test_helpers.expect_usage_error (times ~max_value:(time ~nanosecond:wide 1 0 0) ()) "";
  Test_helpers.expect_usage_error (times ~max_value:(time 1 0 (-1)) ()) "";
  Test_helpers.expect_usage_error
    (datetimes ~max_value:{ date = date 2024 1 1; time = time 256 0 0 } ())
    "";
  Test_helpers.expect_usage_error (instants ~max_value:(instant ~nanosecond:wide 0) ()) "";
  Test_helpers.expect_usage_error
    (durations ~max_value:{ seconds = 1; nanoseconds = wide } ())
    "";
  Test_helpers.expect_usage_error (offsets ~max_value:wide ()) "";
  Test_helpers.expect_usage_error (offsets ~min_value:(-wide) ()) "";
  let source = Z.create () in
  let rule_zone starts : Z.zone_data =
    { initial = aaa
    ; transitions = []
    ; after =
        Rule
          { standard = aaa
          ; dst =
              Some
                { daylight = bbb; starts; ends = { day = Zero_based 300; time = 3600 } }
          }
    }
  in
  Z.add_zone source "Test/Valid" (rule_zone { day = Julian 60; time = 0 });
  List.iter
    (fun starts ->
       expect_usage_error_now "" (fun () ->
         Z.add_zone source "Test/Invalid" (rule_zone starts)))
    [ { day = Julian (0x10000 + 60); time = 0 }
    ; { day = Julian 60; time = wide }
    ; { day = Month_week_day { month = `March; week = 0x100 + 2; weekday = `Sunday }
      ; time = 0
      }
    ];
  expect_usage_error_now "" (fun () ->
    Z.add_zone
      source
      "Test/Invalid"
      { initial = { aaa with offset_seconds = wide }; transitions = []; after = Hold });
  Alcotest.(check bool) "nothing was added" true (info ~source "Test/Invalid" = None)
;;

let test_zones () =
  run (fun tc ->
    let zone = Hegel.draw tc (zones ~allow_links:false ()) in
    match info zone with
    | Some i ->
      Alcotest.(check bool) "not a link" false i.is_link;
      check_string "primary" zone i.primary_name;
      assert (not (List.mem Zone_info.Legacy i.tags))
    | None -> Alcotest.fail "drawn zone unknown");
  let saw_link = ref false in
  let saw_legacy = ref false in
  let saw_unknown = ref false in
  run ~test_cases:300 (fun tc ->
    let zone = Hegel.draw tc (zones ~allow_legacy:true ~allow_unknown:true ()) in
    match info zone with
    | Some i ->
      if i.is_link then saw_link := true;
      if List.mem Zone_info.Legacy i.tags then saw_legacy := true;
      if zone = unknown_zone then saw_unknown := true
    | None -> Alcotest.fail "drawn zone unknown");
  Alcotest.(check bool) "links drawn by default" true !saw_link;
  Alcotest.(check bool) "legacy names drawn" true !saw_legacy;
  Alcotest.(check bool) "unknown zone drawn" true !saw_unknown;
  run (fun tc ->
    let zone = Hegel.draw tc (zones ~names:[ "Asia/Calcutta"; "UTC" ] ()) in
    assert (zone = "Asia/Calcutta" || zone = "UTC"));
  run ~test_cases:10 (fun tc ->
    check_string
      "opaque"
      "Mars/Olympus"
      (Hegel.draw tc (zones ~names:[ "Mars/Olympus" ] ~allow_opaque:true ())));
  Test_helpers.expect_usage_error (zones ~names:[ "Mars/Olympus" ] ()) "Mars/Olympus";
  Test_helpers.expect_usage_error (zones ~names:[] ()) ""
;;

let new_york = "America/New_York"

let consistent (z : Zoned_datetime.t) =
  match z.kind, z.offset with
  | Unique, Known offset ->
    Alcotest.(check int) "instant" (utc_seconds z.civil - offset) z.instant.second;
    Alcotest.(check bool)
      "offsets agree"
      true
      (Offset.equal z.offset_before z.offset_after)
  | Fold, Known offset ->
    Alcotest.(check int) "instant" (utc_seconds z.civil - offset) z.instant.second;
    let expected = if z.fold then z.offset_after else z.offset_before in
    Alcotest.(check bool) "fold picks offset" true (Offset.equal expected z.offset)
  | Gap, Known _ ->
    (match z.offset_before with
     | Known before ->
       Alcotest.(check int) "gap instant" (utc_seconds z.civil - before) z.instant.second
     | Unknown -> Alcotest.fail "gap without an offset")
  | _, Unknown -> ()
;;

let bounded_civil = civil 1900 1 1 0 0, civil 2100 12 31 23 59

let test_zoned_datetimes () =
  let min_value, max_value = bounded_civil in
  let kinds = Hashtbl.create 3 in
  run ~test_cases:1000 (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes
           ~zones:(zones ~names:[ new_york ] ())
           ~min_value
           ~max_value
           ~precision:`Seconds
           ())
    in
    check_string "zone" new_york z.zone;
    Hashtbl.replace kinds z.kind ();
    consistent z);
  Alcotest.(check bool) "gap drawn" true (Hashtbl.mem kinds Zoned_datetime.Gap);
  Alcotest.(check bool) "fold drawn" true (Hashtbl.mem kinds Zoned_datetime.Fold);
  run ~test_cases:200 (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes
           ~zones:(zones ~names:[ new_york ] ())
           ~min_value
           ~max_value
           ~allow_imaginary:false
           ~ambiguous:`Later
           ())
    in
    assert (z.kind <> Gap);
    if z.kind = Fold then assert z.fold);
  run ~test_cases:200 (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes
           ~zones:(zones ~names:[ new_york ] ())
           ~min_value
           ~max_value
           ~ambiguous:`Earlier
           ())
    in
    if z.kind = Fold then assert (not z.fold));
  let min_instant = instant 0 in
  let max_instant = instant (86_400 * 365) in
  run (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ~min_instant ~max_instant ()) in
    assert (
      Instant.compare min_instant z.instant <= 0
      && Instant.compare z.instant max_instant <= 0);
    consistent z);
  run ~test_cases:30 (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ()) in
    assert (Date.compare Date.min_value z.civil.date <= 0));
  expect_usage_error_now "cannot combine" (fun () ->
    zoned_datetimes ~min_value ~max_instant ());
  expect_usage_error_now "cannot combine" (fun () ->
    zoned_datetimes ~min_instant ~max_value ());
  Test_helpers.expect_usage_error
    (zoned_datetimes ~min_value:max_value ~max_value:min_value ())
    "min_value"
;;

let test_zoned_datetimes_any_zone_generator () =
  let min_value, max_value = bounded_civil in
  let names = [ "Asia/Calcutta"; new_york; unknown_zone ] in
  let seen = Hashtbl.create 3 in
  run (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes ~zones:(Hegel.sampled_from names) ~min_value ~max_value ())
    in
    Hashtbl.replace seen z.zone ();
    (match z.zone with
     | "Asia/Calcutta" ->
       Alcotest.(check bool)
         "india"
         true
         (Offset.equal z.offset (Known 19_800) || z.civil.date.year < 1946)
     | "Etc/Unknown" ->
       Alcotest.(check bool) "unknown offset" true (Offset.equal z.offset Unknown)
     | _ -> ());
    consistent z);
  Alcotest.(check int) "every zone drawn" 3 (Hashtbl.length seen);
  let no_such_zone = zoned_datetimes ~zones:(Hegel.just "Mars/Olympus") () in
  Test_helpers.expect_usage_error no_such_zone "unknown time zone \"Mars/Olympus\"";
  Test_helpers.expect_usage_error no_such_zone "~source"
;;

let test_zoned_datetimes_zone_without_a_value () =
  let seen = Hashtbl.create 2 in
  run (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes
           ~zones:(Hegel.sampled_from [ new_york; "UTC" ])
           ~min_value:(civil 2024 3 10 2 10)
           ~max_value:(civil 2024 3 10 2 20)
           ~allow_imaginary:false
           ())
    in
    Hashtbl.replace seen z.zone ());
  Alcotest.(check (list string))
    "only the zone with a wall time in the bounds"
    [ "UTC" ]
    (List.of_seq (Hashtbl.to_seq_keys seen))
;;

let be32 n =
  let b = Bytes.create 4 in
  Bytes.set_int32_be b 0 (Int32.of_int n);
  Bytes.to_string b
;;

let be64 n =
  let b = Bytes.create 8 in
  Bytes.set_int64_be b 0 (Int64.of_int n);
  Bytes.to_string b
;;

let tzif ~types ~transitions ~footer =
  let chars = String.concat "" (List.map (fun (_, _, abbr) -> abbr ^ "\000") types) in
  let _, ttinfos =
    List.fold_left
      (fun (index, acc) (offset, dst, abbr) ->
         ( index + String.length abbr + 1
         , acc
           ^ be32 offset
           ^ (if dst then "\001" else "\000")
           ^ String.make 1 (Char.chr index) ))
      (0, "")
      types
  in
  let block time =
    String.concat
      ""
      [ "TZif2"
      ; String.make 15 '\000'
      ; be32 0
      ; be32 0
      ; be32 0
      ; be32 (List.length transitions)
      ; be32 (List.length types)
      ; be32 (String.length chars)
      ; String.concat "" (List.map (fun (t, _) -> time t) transitions)
      ; String.concat "" (List.map (fun (_, i) -> String.make 1 (Char.chr i)) transitions)
      ; ttinfos
      ; chars
      ]
  in
  block be32 ^ block be64 ^ "\n" ^ footer ^ "\n"
;;

let shift = 1_000_000_000

let shift_tzif =
  tzif
    ~types:[ 0, false, "AAA"; 3600, false, "BBB" ]
    ~transitions:[ shift, 1 ]
    ~footer:"<BBB>-1"
;;

let shift_data : Z.zone_data =
  { initial = aaa; transitions = [ { instant = shift; local_type = bbb } ]; after = Hold }
;;

let in_the_shift = civil 2001 9 9 2 0

let check_shift ?(dst_after = true) source name =
  let gap = resolve ~source name in_the_shift in
  Alcotest.(check bool) (name ^ " gap") true (gap.kind = Gap);
  check_string (name ^ " zone") name gap.zone;
  Alcotest.(check bool) (name ^ " before") true (Offset.equal gap.offset_before (Known 0));
  Alcotest.(check bool)
    (name ^ " after")
    true
    (Offset.equal gap.offset_after (Known 3600));
  let later = resolve ~source name (civil 2010 1 1 0 0) in
  Alcotest.(check bool) (name ^ " later") true (later.kind = Unique);
  Alcotest.(check bool)
    (name ^ " later offset")
    true
    (Offset.equal later.offset (Known 3600));
  let o = offset_at ~source name (instant (shift - 1)) in
  check_string (name ^ " abbreviation before") "AAA" o.abbreviation;
  Alcotest.(check bool) (name ^ " dst before") false o.is_dst;
  let o = offset_at ~source name (instant shift) in
  check_string (name ^ " abbreviation after") "BBB" o.abbreviation;
  Alcotest.(check bool) (name ^ " dst after") dst_after o.is_dst
;;

let oracle_offset t = if t < shift then aaa else bbb

let test_zone_source_data () =
  let source = Z.create () in
  Z.add_tzif source "Test/Tzif" ~contents:shift_tzif;
  Z.add_zone source "Test/Fields" shift_data;
  Z.add_oracle
    source
    "Test/Oracle"
    ~next_transition:(fun t -> if t < shift then Some shift else None)
    oracle_offset;
  Z.add_oracle source "Test/Sampled" ~first_year:1990 ~last_year:2010 oracle_offset;
  Z.add_oracle source "Test/Inferred" ~infer:true oracle_offset;
  Z.add_link source "Test/Alias" ~target:"Test/Fields";
  List.iter
    (check_shift source)
    [ "Test/Fields"; "Test/Oracle"; "Test/Sampled"; "Test/Inferred" ];
  check_shift ~dst_after:false source "Test/Tzif";
  (match info ~source "Test/Alias" with
   | Some i ->
     Alcotest.(check bool) "link" true i.is_link;
     check_string "target" "Test/Fields" i.primary_name;
     assert (List.mem Zone_info.Link i.tags)
   | None -> Alcotest.fail "alias unknown");
  Alcotest.(check bool) "missing" true (info ~source "Test/Missing" = None);
  check_string "alias name" "Test/Alias" (resolve ~source "Test/Alias" in_the_shift).zone;
  check_string "unversioned" "" (Z.version source);
  Z.set_version source "test-1";
  check_string "versioned" "test-1" (Z.version source);
  run (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes
           ~zones:(zones ~source ~names:[ "Test/Tzif"; "Test/Alias" ] ())
           ~min_value:(civil 2001 1 1 0 0)
           ~max_value:(civil 2002 1 1 0 0)
           ())
    in
    assert (z.zone = "Test/Tzif" || z.zone = "Test/Alias");
    consistent z)
;;

let mwd month week weekday : Z.rule_day = Month_week_day { month; week; weekday }

let us_rule : Z.zone_rule =
  { standard = { offset_seconds = -18_000; is_dst = false; abbreviation = "EST" }
  ; dst =
      Some
        { daylight = { offset_seconds = -14_400; is_dst = true; abbreviation = "EDT" }
        ; starts = { day = mwd `March 2 `Sunday; time = 7200 }
        ; ends = { day = mwd `November 1 `Sunday; time = 7200 }
        }
  }
;;

let monday_rule : Z.zone_rule =
  { standard = aaa
  ; dst =
      Some
        { daylight = bbb
        ; starts = { day = mwd `March 1 `Monday; time = 7200 }
        ; ends = { day = mwd `October 5 `Saturday; time = 7200 }
        }
  }
;;

let day_rule : Z.zone_rule =
  { standard = aaa
  ; dst =
      Some
        { daylight = bbb
        ; starts = { day = Julian 60; time = 0 }
        ; ends = { day = Zero_based 300; time = 3600 }
        }
  }
;;

let test_zone_source_rules () =
  let source = Z.create () in
  let rule_zone (rule : Z.zone_rule) : Z.zone_data =
    { initial = rule.standard; transitions = []; after = Rule rule }
  in
  Z.add_zone source "Test/Us" (rule_zone us_rule);
  Z.add_zone source "Test/Monday" (rule_zone monday_rule);
  Z.add_zone source "Test/Days" (rule_zone day_rule);
  Z.add_zone source "Test/Standard" (rule_zone { standard = bbb; dst = None });
  let kind name c = (resolve ~source name c).kind in
  Alcotest.(check bool) "us gap" true (kind "Test/Us" (civil 2030 3 10 2 30) = Gap);
  Alcotest.(check bool) "us fold" true (kind "Test/Us" (civil 2030 11 3 1 30) = Fold);
  Alcotest.(check bool) "us unique" true (kind "Test/Us" (civil 2030 7 1 12 0) = Unique);
  Alcotest.(check bool)
    "first Monday of March"
    true
    (kind "Test/Monday" (civil 2030 3 4 2 30) = Gap);
  Alcotest.(check bool)
    "not the Sunday before"
    true
    (kind "Test/Monday" (civil 2030 3 3 2 30) = Unique);
  Alcotest.(check bool)
    "last Saturday of October"
    true
    (kind "Test/Monday" (civil 2030 10 26 1 30) = Fold);
  Alcotest.(check bool) "julian gap" true (kind "Test/Days" (civil 2030 3 1 0 30) = Gap);
  Alcotest.(check bool)
    "zero based fold"
    true
    (kind "Test/Days" (civil 2030 10 28 0 30) = Fold);
  let standard = resolve ~source "Test/Standard" (civil 2030 7 1 12 0) in
  Alcotest.(check bool) "standard alone" true (Offset.equal standard.offset (Known 3600));
  let years = [ 2018; 2019 ] in
  let transitions =
    List.concat_map
      (fun y ->
         let start = utc_seconds (civil y 3 1 0 0) in
         let stop = utc_seconds (civil y 10 1 0 0) - 3600 in
         [ ({ instant = start; local_type = bbb } : Z.zone_transition)
         ; { instant = stop; local_type = aaa }
         ])
      years
  in
  Z.add_zone source "Test/Infer" { initial = aaa; transitions; after = Infer 2019 };
  Alcotest.(check bool) "inferred gap" true (kind "Test/Infer" (civil 2030 3 1 0 30) = Gap)
;;

exception Custom_failure

let test_zone_source_loader () =
  let source = Z.create () in
  let loaded = ref [] in
  Z.declare source [ "Test/Lazy"; "Test/Broken"; "Test/Empty" ];
  Z.set_loader source (fun s name ->
    loaded := name :: !loaded;
    match name with
    | "Test/Lazy" -> Z.add_zone s name shift_data
    | "Test/Broken" -> failwith "no data for Test/Broken"
    | "Test/Invalid" -> invalid_arg "invalid argument for Test/Invalid"
    | "Test/Usage" -> raise (Hegel.Usage_error "usage error for Test/Usage")
    | "Test/Custom" -> raise Custom_failure
    | "Test/Bytes" -> failwith "\xff\xfe"
    | _ -> ());
  Alcotest.(check (list string)) "nothing loaded yet" [] !loaded;
  check_shift source "Test/Lazy";
  Alcotest.(check bool) "loaded" true (List.mem "Test/Lazy" !loaded);
  Alcotest.(check bool) "undeclared is unknown" true (info ~source "Test/Other" = None);
  expect_usage_error_now "supplied no data" (fun () -> info ~source "Test/Empty");
  expect_usage_error_now "no data for Test/Broken" (fun () -> info ~source "Test/Broken");
  expect_usage_error_now "invalid argument for Test/Invalid" (fun () ->
    info ~source "Test/Invalid");
  expect_usage_error_now "usage error for Test/Usage" (fun () ->
    info ~source "Test/Usage");
  expect_usage_error_now "Custom_failure" (fun () -> info ~source "Test/Custom");
  expect_usage_error_now "loader" (fun () -> info ~source "Test/Bytes");
  run ~test_cases:20 (fun tc ->
    check_string
      "a name whose sibling fails to load"
      "Test/Lazy"
      (Hegel.draw tc (zones ~source ~names:[ "Test/Lazy" ] ())));
  Test_helpers.expect_usage_error (zones ~source ()) "no data for Test/Broken";
  let source = Z.create () in
  Z.declare source [ "Test/Lazy" ];
  Z.set_loader source (fun s name -> Z.add_zone s name shift_data);
  run ~test_cases:20 (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ~source ()) in
    check_string "lazy zone" "Test/Lazy" z.zone)
;;

let test_zone_source_loader_replaced () =
  let source = Z.create () in
  let calls = ref [] in
  let loader tag = Z.set_loader source (fun _ name -> calls := (tag, name) :: !calls) in
  loader "first";
  Alcotest.(check bool) "first asked" true (info ~source "Test/A" = None);
  loader "second";
  Alcotest.(check bool) "second asked" true (info ~source "Test/B" = None);
  Z.remove_loader source;
  Alcotest.(check bool) "nothing asked" true (info ~source "Test/C" = None);
  Alcotest.(check (list (pair string string)))
    "each loader was asked while it was set"
    [ "second", "Test/B"; "first", "Test/A" ]
    !calls;
  expect_usage_error_now "" (fun () -> Z.set_loader (Z.bundled ()) (fun _ _ -> ()));
  expect_usage_error_now "" (fun () -> Z.remove_loader (Z.bundled ()))
;;

let test_zone_source_loader_released () =
  let source = Z.create () in
  let released = Atomic.make 0 in
  let set_loader () =
    let captured = Bytes.create 64 in
    Gc.finalise (fun _ -> Atomic.incr released) captured;
    Z.set_loader source (fun _ _ -> ignore (Sys.opaque_identity captured : Bytes.t))
  in
  let live_after f =
    f ();
    Gc.full_major ();
    Gc.full_major ();
    Atomic.get released
  in
  Alcotest.(check int) "kept while it is the loader" 0 (live_after set_loader);
  Alcotest.(check int) "released when replaced" 1 (live_after set_loader);
  Alcotest.(check int)
    "released when removed"
    2
    (live_after (fun () -> Z.remove_loader source));
  Alcotest.(check int)
    "released when the call fails"
    3
    (live_after (fun () ->
       let captured = Bytes.create 64 in
       Gc.finalise (fun _ -> Atomic.incr released) captured;
       expect_usage_error_now "" (fun () ->
         Z.set_loader (Z.bundled ()) (fun _ _ ->
           ignore (Sys.opaque_identity captured : Bytes.t)))))
;;

let test_zone_source_errors () =
  let bundled = Z.bundled () in
  check_string "bundled version" (tzdb_version ()) (Z.version bundled);
  Alcotest.(check bool) "release" true (String.length (tzdb_version ()) >= 5);
  expect_usage_error_now "" (fun () -> Z.add_link bundled "Test/X" ~target:"UTC");
  let source = Z.create () in
  expect_usage_error_now "" (fun () ->
    Z.add_tzif source "Test/Bad" ~contents:"not a tzif file");
  expect_usage_error_now "" (fun () -> Z.add_link source unknown_zone ~target:"UTC");
  expect_usage_error_now "oracle exploded" (fun () ->
    Z.add_oracle source "Test/Raises" (fun _ -> failwith "oracle exploded"));
  expect_usage_error_now "next exploded" (fun () ->
    Z.add_oracle
      source
      "Test/Raises"
      ~next_transition:(fun _ -> invalid_arg "next exploded")
      oracle_offset);
  expect_usage_error_now "" (fun () ->
    Z.add_oracle source "Test/Years" ~first_year:(wide + 1900) oracle_offset);
  expect_usage_error_now "" (fun () ->
    Z.add_oracle source "Test/Years" ~last_year:(wide + 2100) oracle_offset);
  expect_usage_error_now "" (fun () ->
    Z.add_oracle source "Test/Wide" (fun _ -> { aaa with offset_seconds = wide }));
  expect_usage_error_now "" (fun () ->
    resolve ~source "Test/Missing" (civil 2000 1 1 0 0));
  expect_usage_error_now "" (fun () -> offset_at ~source "Test/Missing" (instant 0))
;;

let test_queries () =
  let z = resolve new_york (civil 2024 3 10 2 30) in
  Alcotest.(check bool) "gap" true (z.kind = Gap);
  Alcotest.(check bool) "before" true (Offset.equal z.offset_before (Known (-18_000)));
  Alcotest.(check bool) "after" true (Offset.equal z.offset_after (Known (-14_400)));
  let z = resolve new_york (civil 2024 11 3 1 30) in
  Alcotest.(check bool) "fold" true (z.kind = Fold);
  Alcotest.(check bool) "earlier" false z.fold;
  Alcotest.(check bool) "earlier offset" true (Offset.equal z.offset (Known (-14_400)));
  let z = resolve unknown_zone (civil 2024 1 1 0 0) in
  Alcotest.(check bool) "unknown offset" true (Offset.equal z.offset Unknown);
  let o = offset_at new_york (instant 1_720_000_000) in
  check_string "summer" "EDT" o.abbreviation;
  Alcotest.(check bool) "summer offset" true (Offset.equal o.offset (Known (-14_400)));
  (match info "Asia/Calcutta" with
   | Some i -> check_string "link primary" "Asia/Kolkata" i.primary_name
   | None -> Alcotest.fail "Asia/Calcutta unknown");
  (match info "UTC" with
   | Some i -> Alcotest.(check int) "utc rank" 0 i.rank
   | None -> Alcotest.fail "UTC unknown");
  match info unknown_zone with
  | Some i -> assert (List.mem Zone_info.Undefined i.tags)
  | None -> Alcotest.fail "unknown zone unknown"
;;

let test_zoned_datetimes_source () =
  let source = Z.create () in
  Z.add_zone source "Test/Fields" shift_data;
  Z.add_zone source "Test/Fixed" { initial = aaa; transitions = []; after = Hold };
  let names = [ "Test/Fields"; "Test/Fixed" ] in
  run (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ~source ()) in
    assert (List.mem z.zone names));
  let derived = Hegel.filter (fun name -> name <> "Test/Fixed") (zones ~source ()) in
  let saw_gap = ref false in
  run (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes
           ~zones:derived
           ~source
           ~min_value:(civil 2001 9 9 1 0)
           ~max_value:(civil 2001 9 9 4 0)
           ())
    in
    check_string "derived zone" "Test/Fields" z.zone;
    if z.kind = Gap then saw_gap := true;
    consistent z);
  Alcotest.(check bool) "the source's gap is drawn" true !saw_gap;
  let without_source = zoned_datetimes ~zones:derived () in
  Test_helpers.expect_usage_error without_source "unknown time zone \"Test/Fields\"";
  Test_helpers.expect_usage_error without_source "~source";
  expect_usage_error_now "pass ~source to Temporal.zones" (fun () ->
    zoned_datetimes ~zones:(zones ~source ()) ~source ());
  expect_usage_error_now "pass ~source to Temporal.zones" (fun () ->
    zoned_datetimes ~zones:(zones ()) ~source ())
;;

let test_default_zone_source () =
  check_string "bundled default" (tzdb_version ()) (Z.version (default_zone_source ()));
  let source = Z.create () in
  Z.add_zone source "Test/Only" shift_data;
  Z.set_version source "custom";
  let only = Hegel.just "Test/Only" in
  let at_the_shift zones =
    zoned_datetimes ?zones ~min_value:in_the_shift ~max_value:in_the_shift ()
  in
  let zones_before = zones () in
  let zoned_before = zoned_datetimes () in
  let derived_before = at_the_shift (Some only) in
  let broken = Z.create () in
  Z.declare broken [ "Test/Broken" ];
  let previous = set_default_zone_source source in
  let zones_during, zoned_during, derived_during =
    Fun.protect
      ~finally:(fun () ->
        check_string
          "set returns the default it replaced"
          "custom"
          (Z.version (set_default_zone_source previous)))
      (fun () ->
         check_string "previous default" (tzdb_version ()) (Z.version previous);
         check_string "custom default" "custom" (Z.version (default_zone_source ()));
         Alcotest.(check bool) "queries read the default" true (info "Test/Only" <> None);
         check_shift (default_zone_source ()) "Test/Only";
         let zones_during = zones () in
         let zoned_during = at_the_shift None in
         let derived_during = at_the_shift (Some only) in
         run ~test_cases:20 (fun tc ->
           check_string "zones" "Test/Only" (Hegel.draw tc zones_during);
           let z = Hegel.draw tc zoned_during in
           check_string "zoned" "Test/Only" z.zone;
           Alcotest.(check bool) "zoned kind" true (z.kind = Gap);
           let z = Hegel.draw tc derived_during in
           Alcotest.(check bool) "derived kind" true (z.kind = Gap);
           assert (Hegel.draw tc zones_before <> "Test/Only");
           assert ((Hegel.draw tc zoned_before).zone <> "Test/Only"));
         Test_helpers.expect_usage_error derived_before "unknown time zone \"Test/Only\"";
         check_string "replaced" "custom" (Z.version (set_default_zone_source broken));
         run ~test_cases:20 (fun tc ->
           let zone = Hegel.draw tc (zones ~source:(Z.bundled ()) ()) in
           assert (info ~source:(Z.bundled ()) zone <> None));
         check_string "broken default" "" (Z.version (set_default_zone_source source));
         zones_during, zoned_during, derived_during)
  in
  check_string "restored" (tzdb_version ()) (Z.version (default_zone_source ()));
  Alcotest.(check bool) "queries read the restored default" true (info "Test/Only" = None);
  run ~test_cases:20 (fun tc ->
    check_string "zones keep their source" "Test/Only" (Hegel.draw tc zones_during);
    check_string "zoned keep their source" "Test/Only" (Hegel.draw tc zoned_during).zone;
    Alcotest.(check bool)
      "derived keep their source"
      true
      ((Hegel.draw tc derived_during).kind = Gap))
;;

let live_specs = Hegel.Generators.Private.live_temporal_specs

let settled_live_specs () =
  Gc.full_major ();
  Gc.full_major ();
  live_specs ()
;;

let test_specs_freed_with_generator () =
  let base = settled_live_specs () in
  let draw_once () =
    let dates = dates () in
    let spans = spans ~days:(0, 3) () in
    let zoned = zoned_datetimes () in
    for _ = 1 to 2 do
      run ~test_cases:3 (fun tc ->
        ignore (Hegel.draw tc dates : Date.t);
        ignore (Hegel.draw tc spans : Span.t);
        ignore (Hegel.draw tc zoned : Zoned_datetime.t))
    done;
    Alcotest.(check int) "one configuration each, built once" (base + 4) (live_specs ())
  in
  draw_once ();
  Alcotest.(check int) "freed with their generators" base (settled_live_specs ());
  Test_helpers.expect_usage_error (spans ~days:(5, 1) ()) "min_value";
  Test_helpers.expect_usage_error (zones ~names:[ "Mars/Olympus" ] ()) "Mars/Olympus";
  Test_helpers.expect_usage_error
    (zoned_datetimes ~min_value:(civil 2001 1 1 0 0) ~max_value:(civil 2000 1 1 0 0) ())
    "min_value";
  Test_helpers.expect_usage_error (dates ~weekdays:[] ()) "weekday";
  Alcotest.(check int) "a rejected configuration is not kept" base (settled_live_specs ())
;;

let test_shared_across_domains () =
  let source = Z.create () in
  Z.declare source [ "Test/Lazy" ];
  Z.set_loader source (fun s name -> Z.add_zone s name shift_data);
  let zoned = zoned_datetimes ~source () in
  let spans = spans ~days:(0, 3) () in
  let work () =
    run ~test_cases:50 (fun tc ->
      assert ((Hegel.draw tc zoned).zone = "Test/Lazy");
      assert ((Hegel.draw tc spans).days <= 3))
  in
  List.init 4 (fun _ ->
    (Domain.spawn [@alert "-do_not_spawn_domains-unsafe_multidomain"]) work)
  |> List.iter Domain.join
;;

let tests =
  [ Alcotest.test_case "default zone source" `Quick test_default_zone_source
  ; Alcotest.test_case "specs freed with generator" `Quick test_specs_freed_with_generator
  ; Alcotest.test_case "date text" `Quick test_date_text
  ; Alcotest.test_case "time text" `Quick test_time_text
  ; Alcotest.test_case "instant text" `Quick test_instant_text
  ; Alcotest.test_case "instant text matches core" `Quick test_instant_text_matches_core
  ; Alcotest.test_case "offset text" `Quick test_offset_text
  ; Alcotest.test_case "zoned text" `Quick test_zoned_text
  ; Alcotest.test_case "duration text" `Quick test_duration_text
  ; Alcotest.test_case "span text" `Quick test_span_text
  ; Alcotest.test_case "equal and compare" `Quick test_order
  ; Alcotest.test_case "default generators" `Quick test_default_generators
  ; Alcotest.test_case "failure output" `Quick test_failure_output
  ; Alcotest.test_case "dates" `Quick test_dates
  ; Alcotest.test_case "dates invalid" `Quick test_dates_invalid
  ; Alcotest.test_case "times" `Quick test_times
  ; Alcotest.test_case "datetimes" `Quick test_datetimes
  ; Alcotest.test_case "instants" `Quick test_instants
  ; Alcotest.test_case "offsets" `Quick test_offsets
  ; Alcotest.test_case "durations" `Quick test_durations
  ; Alcotest.test_case "spans" `Quick test_spans
  ; Alcotest.test_case
      "out of range fields are rejected"
      `Quick
      test_out_of_range_fields_are_rejected
  ; Alcotest.test_case "zones" `Quick test_zones
  ; Alcotest.test_case "zoned datetimes" `Quick test_zoned_datetimes
  ; Alcotest.test_case
      "zoned datetimes any zone generator"
      `Quick
      test_zoned_datetimes_any_zone_generator
  ; Alcotest.test_case
      "zoned datetimes zone without a value"
      `Quick
      test_zoned_datetimes_zone_without_a_value
  ; Alcotest.test_case "zoned datetimes source" `Quick test_zoned_datetimes_source
  ; Alcotest.test_case "zone source data" `Quick test_zone_source_data
  ; Alcotest.test_case "zone source rules" `Quick test_zone_source_rules
  ; Alcotest.test_case "zone source loader" `Quick test_zone_source_loader
  ; Alcotest.test_case
      "zone source loader replaced"
      `Quick
      test_zone_source_loader_replaced
  ; Alcotest.test_case
      "zone source loader released"
      `Quick
      test_zone_source_loader_released
  ; Alcotest.test_case "zone source errors" `Quick test_zone_source_errors
  ; Alcotest.test_case "queries" `Quick test_queries
  ; Alcotest.test_case "shared across domains" `Quick test_shared_across_domains
  ]
;;
