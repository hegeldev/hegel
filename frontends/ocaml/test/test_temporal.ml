open Hegel.Temporal
module Z = Zone_source

let settings ?(test_cases = 100) () = Hegel.Settings.create ~test_cases ~seed:0 ()
let run ?test_cases f = Hegel.run_hegel_test ~settings:(settings ?test_cases ()) f

let print sexp_of v =
  match sexp_of v with
  | Sexplib0.Sexp.Atom s -> s
  | List _ -> Alcotest.fail "printer produced a list"
;;

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

let core_date ({ year; month; day } : date) =
  Core.Date.create_exn ~y:year ~m:(Core.Month.of_int_exn month) ~d:day
;;

let epoch = Core.Date.create_exn ~y:1970 ~m:Jan ~d:1

let utc_seconds ({ date; time } : datetime) =
  (Core.Date.diff (core_date date) epoch * 86_400)
  + (time.hour * 3600)
  + (time.minute * 60)
  + time.second
;;

let test_date_printer () =
  check_string
    "plain"
    "2024-03-10"
    (print sexp_of_date { year = 2024; month = 3; day = 10 });
  check_string "padded" "0007-01-02" (print sexp_of_date { year = 7; month = 1; day = 2 });
  check_string
    "large"
    "+010000-12-31"
    (print sexp_of_date { year = 10_000; month = 12; day = 31 });
  check_string
    "negative"
    "-000001-06-15"
    (print sexp_of_date { year = -1; month = 6; day = 15 })
;;

let test_time_printer () =
  check_string
    "whole"
    "02:30:00"
    (print sexp_of_time { hour = 2; minute = 30; second = 0; nanosecond = 0 });
  check_string
    "half"
    "23:59:59.5"
    (print sexp_of_time { hour = 23; minute = 59; second = 59; nanosecond = 500_000_000 });
  check_string
    "nano"
    "00:00:00.000000001"
    (print sexp_of_time { hour = 0; minute = 0; second = 0; nanosecond = 1 });
  check_string
    "datetime"
    "2024-03-10T02:30:00.12"
    (print
       sexp_of_datetime
       { date = { year = 2024; month = 3; day = 10 }
       ; time = { hour = 2; minute = 30; second = 0; nanosecond = 120_000_000 }
       })
;;

let test_instant_printer () =
  let instant second nanosecond = print sexp_of_instant { second; nanosecond } in
  check_string "epoch" "1970-01-01T00:00:00Z" (instant 0 0);
  check_string "y2k" "2000-01-01T00:00:00Z" (instant 946_684_800 0);
  check_string "before epoch" "1969-12-31T23:59:58.5Z" (instant (-2) 500_000_000);
  check_string "leap day" "2024-02-29T12:00:00Z" (instant 1_709_208_000 0);
  check_string "min" "-999999-01-01T00:00:00Z" (instant min_instant 0);
  check_string "max" "+999999-12-31T23:59:59.999999999Z" (instant max_instant 999_999_999);
  check_string "year zero" "0000-03-01T00:00:00Z" (instant (-62_162_035_200) 0);
  check_string "before year zero" "-000001-12-31T00:00:00Z" (instant (-62_167_305_600) 0)
;;

let test_instant_printer_matches_core () =
  run (fun tc ->
    let i =
      Hegel.draw
        tc
        (instants
           ~min_value:{ second = -62_135_596_800; nanosecond = 0 }
           ~max_value:{ second = 253_402_300_799; nanosecond = 0 }
           ~precision:Seconds
           ())
    in
    let days =
      Int.div (i.second - (((i.second mod 86_400) + 86_400) mod 86_400)) 86_400
    in
    let expected = Core.Date.to_string (Core.Date.add_days epoch days) in
    let printed = print sexp_of_instant i in
    check_string "date part" expected (String.sub printed 0 10))
;;

let test_offset_printer () =
  check_string "unknown" "-00:00" (print sexp_of_utc_offset Unknown);
  check_string "zero" "+00:00" (print sexp_of_utc_offset (Known 0));
  check_string "india" "+05:30" (print sexp_of_utc_offset (Known 19_800));
  check_string "amsterdam" "+00:19:32" (print sexp_of_utc_offset (Known 1172));
  check_string "monrovia" "-00:44:30" (print sexp_of_utc_offset (Known (-2670)));
  check_string "max" "+25:59:59" (print sexp_of_utc_offset (Known max_offset))
;;

let zoned ?(name = "America/New_York") ?(kind = Unique) ?(fold = false) offset =
  { zone = { name; primary_name = name }
  ; civil =
      { date = { year = 2024; month = 3; day = 10 }
      ; time = { hour = 1; minute = 30; second = 0; nanosecond = 0 }
      }
  ; kind
  ; offset_before = offset
  ; offset_after = offset
  ; fold
  ; offset
  ; instant = { second = 0; nanosecond = 0 }
  }
;;

let test_zoned_printer () =
  let p z = print sexp_of_zoned_datetime z in
  check_string
    "unique"
    "2024-03-10T01:30:00-05:00[America/New_York]"
    (p (zoned (Known (-18_000))));
  check_string
    "gap"
    "2024-03-10T01:30:00-05:00[America/New_York][gap]"
    (p (zoned ~kind:Gap (Known (-18_000))));
  check_string
    "fold 0"
    "2024-03-10T01:30:00-04:00[America/New_York][fold=0]"
    (p (zoned ~kind:Fold (Known (-14_400))));
  check_string
    "fold 1"
    "2024-03-10T01:30:00-05:00[America/New_York][fold=1]"
    (p (zoned ~kind:Fold ~fold:true (Known (-18_000))));
  check_string
    "unknown zone"
    "2024-03-10T01:30:00Z[Etc/Unknown]"
    (p (zoned ~name:unknown_zone Unknown));
  check_string
    "opaque zone"
    "2024-03-10T01:30:00-00:00[Mars/Olympus]"
    (p (zoned ~name:"Mars/Olympus" Unknown));
  check_string
    "zone"
    "Asia/Calcutta"
    (print sexp_of_time_zone { name = "Asia/Calcutta"; primary_name = "Asia/Kolkata" })
;;

let test_duration_printer () =
  let p seconds nanoseconds = print sexp_of_signed_duration { seconds; nanoseconds } in
  check_string "zero" "PT0S" (p 0 0);
  check_string "mixed" "PT1H30M5.5S" (p 5405 500_000_000);
  check_string "hours" "PT2H" (p 7200 0);
  check_string "minutes" "PT1M" (p 60 0);
  check_string "hours seconds" "PT1H1S" (p 3601 0);
  check_string "negative" "-PT1.5S" (p (-1) (-500_000_000));
  check_string "negative fraction" "-PT0.000000001S" (p 0 (-1));
  check_string "days" "PT48H" (p 172_800 0)
;;

let span =
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

let test_span_printer () =
  let p s = print sexp_of_span s in
  check_string "zero" "0s" (p span);
  check_string
    "all"
    "1y 2mo 3w 4d 5h 6m 7s 8ms 9\xc2\xb5s 10ns"
    (p
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
       });
  check_string "negative" "-1y 2d" (p { span with years = -1; days = -2 });
  check_string "mixed" "1y -2d" (p { span with years = 1; days = -2 })
;;

let test_dates () =
  let min_value = { year = 1; month = 1; day = 1 } in
  let max_value = { year = 9999; month = 12; day = 31 } in
  run (fun tc ->
    let d =
      Hegel.draw
        tc
        (dates
           ~min_value
           ~max_value
           ~weekdays:[ Saturday; Sunday ]
           ~months:[ February ]
           ())
    in
    let core = core_date d in
    Alcotest.(check int) "month" 2 d.month;
    Alcotest.(check bool)
      "weekend"
      true
      (Core.Day_of_week.is_sun_or_sat (Core.Date.day_of_week core)));
  run (fun tc ->
    let lo = Hegel.draw tc (dates ~min_value ~max_value ()) in
    let hi = Hegel.draw tc (dates ~min_value:lo ~max_value ()) in
    let d = Hegel.draw tc (dates ~min_value:lo ~max_value:hi ()) in
    assert (compare lo d <= 0 && compare d hi <= 0);
    ignore (core_date d : Core.Date.t));
  let saw_extended = ref false in
  run (fun tc ->
    let d = Hegel.draw tc (dates ()) in
    assert (d.year >= min_year && d.year <= max_year);
    if d.year < 0 || d.year > 9999 then saw_extended := true);
  Alcotest.(check bool) "default range is the engine's" true !saw_extended
;;

let test_dates_invalid () =
  Test_helpers.expect_usage_error (dates ~weekdays:[] ()) "weekday";
  Test_helpers.expect_usage_error (dates ~months:[] ()) "month";
  Test_helpers.expect_usage_error
    (dates
       ~min_value:{ year = 2024; month = 1; day = 2 }
       ~max_value:{ year = 2024; month = 1; day = 1 }
       ())
    "min_value <= max_value"
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
    [ (Nanoseconds : precision), 1
    ; Microseconds, 1_000
    ; Milliseconds, 1_000_000
    ; Seconds, 1_000_000_000
    ; Minutes, 60_000_000_000
    ; Hours, 3_600_000_000_000
    ];
  run (fun tc ->
    let min_value = { hour = 9; minute = 0; second = 0; nanosecond = 0 } in
    let max_value = { hour = 17; minute = 0; second = 0; nanosecond = 0 } in
    let t = Hegel.draw tc (times ~min_value ~max_value ()) in
    assert (compare min_value t <= 0 && compare t max_value <= 0))
;;

let test_datetimes () =
  let min_value =
    { date = { year = 2024; month = 3; day = 10 }
    ; time = { hour = 12; minute = 0; second = 0; nanosecond = 0 }
    }
  in
  let max_value = { min_value with date = { year = 2024; month = 3; day = 11 } } in
  run (fun tc ->
    let dt = Hegel.draw tc (datetimes ~min_value ~max_value ~precision:Minutes ()) in
    assert (compare min_value dt <= 0 && compare dt max_value <= 0);
    Alcotest.(check int) "seconds" 0 dt.time.second);
  run ~test_cases:20 (fun tc ->
    let dt = Hegel.draw tc (datetimes ()) in
    assert (dt.date.year >= min_year && dt.date.year <= max_year))
;;

let test_instants () =
  let min_value : instant = { second = -86_400; nanosecond = 0 } in
  let max_value : instant = { second = 86_400; nanosecond = 0 } in
  run (fun tc ->
    let i = Hegel.draw tc (instants ~min_value ~max_value ~precision:Milliseconds ()) in
    assert (compare min_value i <= 0 && compare i max_value <= 0);
    Alcotest.(check int) "millis" 0 (i.nanosecond mod 1_000_000));
  run ~test_cases:20 (fun tc ->
    let i = Hegel.draw tc (instants ()) in
    assert (i.second >= min_instant && i.second <= max_instant))
;;

let test_offsets () =
  List.iter
    (fun (granularity, unit) ->
       run ~test_cases:30 (fun tc ->
         match Hegel.draw tc (offsets ~granularity ()) with
         | Known s ->
           Alcotest.(check int) "granularity" 0 (s mod unit);
           assert (abs s <= max_offset)
         | Unknown -> Alcotest.fail "unknown offset without ~unknown"))
    [ (Hours : granularity), 3600; Quarter_hours, 900; Minutes, 60; Seconds, 1 ];
  let saw_unknown = ref false in
  run (fun tc ->
    match
      Hegel.draw
        tc
        (offsets ~min_value:(Known (-3600)) ~max_value:(Known 3600) ~unknown:true ())
    with
    | Unknown -> saw_unknown := true
    | Known s -> assert (abs s <= 3600));
  Alcotest.(check bool) "unknown drawn" true !saw_unknown;
  expect_usage_error_now "min_value" (fun () -> offsets ~min_value:Unknown ());
  expect_usage_error_now "max_value" (fun () -> offsets ~max_value:Unknown ())
;;

let test_durations () =
  let min_value : signed_duration = { seconds = -10; nanoseconds = 0 } in
  let max_value : signed_duration = { seconds = 10; nanoseconds = 0 } in
  run (fun tc ->
    let d = Hegel.draw tc (durations ~min_value ~max_value ~precision:Microseconds ()) in
    assert (compare min_value d <= 0 && compare d max_value <= 0);
    assert (
      (d.seconds <= 0 && d.nanoseconds <= 0) || (d.seconds >= 0 && d.nanoseconds >= 0));
    Alcotest.(check int) "micros" 0 (d.nanoseconds mod 1000));
  run ~test_cases:20 (fun tc ->
    let d = Hegel.draw tc (durations ()) in
    assert (abs d.nanoseconds <= 999_999_999))
;;

let span_values (s : span) =
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
    let s = Hegel.draw tc (spans ~sign:Non_negative ()) in
    assert (List.for_all (fun v -> v >= 0) (span_values s)));
  run (fun tc ->
    let s = Hegel.draw tc (spans ()) in
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
              ~sign:Mixed
              ()))
    in
    if List.exists (fun v -> v > 0) values && List.exists (fun v -> v < 0) values
    then saw_mixed := true);
  Alcotest.(check bool) "mixed signs drawn" true !saw_mixed;
  run ~test_cases:10 (fun tc -> ignore (Hegel.draw tc (spans ~sign:Mixed ()) : span));
  Test_helpers.expect_usage_error (spans ~days:(5, 1) ()) "Days"
;;

let test_zones () =
  run (fun tc ->
    let z = Hegel.draw tc (zones ~links:false ()) in
    match info z.name with
    | Some i ->
      Alcotest.(check bool) "not a link" false i.is_link;
      check_string "primary" z.name z.primary_name;
      assert (not (List.mem Legacy i.tags))
    | None -> Alcotest.fail "drawn zone unknown");
  let saw_link = ref false in
  run ~test_cases:300 (fun tc ->
    let z = Hegel.draw tc (zones ~legacy:true ~unknown:true ()) in
    if z.name <> z.primary_name then saw_link := true);
  Alcotest.(check bool) "links drawn by default" true !saw_link;
  run (fun tc ->
    let z = Hegel.draw tc (zones ~names:[ "Asia/Calcutta"; "UTC" ] ()) in
    match z.name with
    | "Asia/Calcutta" -> check_string "link resolved" "Asia/Kolkata" z.primary_name
    | name -> check_string "utc" "UTC" name);
  run ~test_cases:10 (fun tc ->
    let z = Hegel.draw tc (zones ~names:[ "Mars/Olympus" ] ~opaque:true ()) in
    check_string "opaque" "Mars/Olympus" z.primary_name);
  Test_helpers.expect_usage_error (zones ~names:[ "Mars/Olympus" ] ()) "Mars/Olympus"
;;

let new_york = "America/New_York"

let consistent (z : zoned_datetime) =
  match z.kind, z.offset with
  | Unique, Known offset ->
    Alcotest.(check int) "instant" (utc_seconds z.civil - offset) z.instant.second;
    Alcotest.(check bool) "offsets agree" true (z.offset_before = z.offset_after)
  | Fold, Known offset ->
    Alcotest.(check int) "instant" (utc_seconds z.civil - offset) z.instant.second;
    let expected = if z.fold then z.offset_after else z.offset_before in
    Alcotest.(check bool) "fold picks offset" true (expected = z.offset)
  | Gap, Known _ ->
    (match z.offset_before with
     | Known before ->
       Alcotest.(check int) "gap instant" (utc_seconds z.civil - before) z.instant.second
     | Unknown -> Alcotest.fail "gap without an offset")
  | _, Unknown -> ()
;;

let bounded_civil =
  ( { date = { year = 1900; month = 1; day = 1 }
    ; time = { hour = 0; minute = 0; second = 0; nanosecond = 0 }
    }
  , { date = { year = 2100; month = 12; day = 31 }
    ; time = { hour = 23; minute = 59; second = 59; nanosecond = 0 }
    } )
;;

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
           ~precision:Seconds
           ())
    in
    check_string "zone" new_york z.zone.name;
    Hashtbl.replace kinds z.kind ();
    consistent z);
  Alcotest.(check bool) "gap drawn" true (Hashtbl.mem kinds Gap);
  Alcotest.(check bool) "fold drawn" true (Hashtbl.mem kinds Fold);
  run ~test_cases:200 (fun tc ->
    let z =
      Hegel.draw
        tc
        (zoned_datetimes
           ~zones:(zones ~names:[ new_york ] ())
           ~min_value
           ~max_value
           ~allow_imaginary:false
           ~ambiguous:Later
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
           ~ambiguous:Earlier
           ())
    in
    if z.kind = Fold then assert (not z.fold));
  let min_instant : instant = { second = 0; nanosecond = 0 } in
  let max_instant : instant = { second = 86_400 * 365; nanosecond = 0 } in
  run (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ~min_instant ~max_instant ()) in
    assert (compare min_instant z.instant <= 0 && compare z.instant max_instant <= 0);
    consistent z);
  run ~test_cases:30 (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ()) in
    assert (z.civil.date.year >= min_year));
  expect_usage_error_now "cannot combine" (fun () ->
    zoned_datetimes ~min_value ~max_instant ())
;;

let test_zoned_datetimes_any_zone_generator () =
  let min_value, max_value = bounded_civil in
  let names = [ "Asia/Calcutta"; new_york; "Mars/Olympus" ] in
  let picked =
    Hegel.with_printer
      sexp_of_time_zone
      (Hegel.map (fun name -> { name; primary_name = name }) (Hegel.sampled_from names))
  in
  let seen = Hashtbl.create 3 in
  run (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ~zones:picked ~min_value ~max_value ()) in
    Hashtbl.replace seen z.zone.name ();
    (match z.zone.name with
     | "Asia/Calcutta" ->
       check_string "primary" "Asia/Kolkata" z.zone.primary_name;
       Alcotest.(check bool)
         "india"
         true
         (z.offset = Known 19_800 || z.civil.date.year < 1946)
     | "Mars/Olympus" ->
       check_string "opaque primary" "Mars/Olympus" z.zone.primary_name;
       Alcotest.(check bool) "opaque offset" true (z.offset = Unknown)
     | _ -> ());
    consistent z);
  Alcotest.(check int) "every zone drawn" 3 (Hashtbl.length seen)
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
let aaa : Z.zone_type = { offset_seconds = 0; is_dst = false; abbreviation = "AAA" }
let bbb : Z.zone_type = { offset_seconds = 3600; is_dst = true; abbreviation = "BBB" }

let shift_tzif =
  tzif
    ~types:[ 0, false, "AAA"; 3600, true, "BBB" ]
    ~transitions:[ shift, 1 ]
    ~footer:"<BBB>-1"
;;

let shift_data : Z.zone_data =
  { initial = aaa; transitions = [ { instant = shift; local_type = bbb } ]; after = Hold }
;;

let civil y mo d h mi : datetime =
  { date = { year = y; month = mo; day = d }
  ; time = { hour = h; minute = mi; second = 0; nanosecond = 0 }
  }
;;

let check_shift ?(dst_after = true) source name =
  let gap = resolve ~source name (civil 2001 9 9 2 0) in
  Alcotest.(check bool) (name ^ " gap") true (gap.kind = Gap);
  check_string (name ^ " zone") name gap.zone.name;
  Alcotest.(check bool) (name ^ " before") true (gap.offset_before = Known 0);
  Alcotest.(check bool) (name ^ " after") true (gap.offset_after = Known 3600);
  let later = resolve ~source name (civil 2010 1 1 0 0) in
  Alcotest.(check bool) (name ^ " later") true (later.kind = Unique);
  Alcotest.(check bool) (name ^ " later offset") true (later.offset = Known 3600);
  let o = offset_at ~source name { second = shift - 1; nanosecond = 0 } in
  check_string (name ^ " abbreviation before") "AAA" o.abbreviation;
  Alcotest.(check bool) (name ^ " dst before") false o.is_dst;
  let o = offset_at ~source name { second = shift; nanosecond = 0 } in
  check_string (name ^ " abbreviation after") "BBB" o.abbreviation;
  Alcotest.(check bool) (name ^ " dst after") dst_after o.is_dst
;;

let oracle_offset t = if t < shift then aaa else bbb

let test_zone_source_data () =
  let source = Z.create () in
  Z.add_tzif source "Test/Tzif" shift_tzif;
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
     assert (List.mem Link i.tags)
   | None -> Alcotest.fail "alias unknown");
  Alcotest.(check bool) "missing" true (info ~source "Test/Missing" = None);
  let alias = resolve ~source "Test/Alias" (civil 2001 9 9 2 0) in
  check_string "alias primary" "Test/Fields" alias.zone.primary_name;
  check_string "alias name" "Test/Alias" alias.zone.name;
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
    if z.zone.name = "Test/Alias"
    then check_string "primary" "Test/Fields" z.zone.primary_name;
    consistent z)
;;

let mwd month week weekday : Z.rule_day = Month_week_day { month; week; weekday }

let us_rule : Z.zone_rule =
  { standard = { offset_seconds = -18_000; is_dst = false; abbreviation = "EST" }
  ; dst =
      Some
        { local_type = { offset_seconds = -14_400; is_dst = true; abbreviation = "EDT" }
        ; starts = { day = mwd March 2 Sunday; time = 7200 }
        ; ends = { day = mwd November 1 Sunday; time = 7200 }
        }
  }
;;

let day_rule : Z.zone_rule =
  { standard = aaa
  ; dst =
      Some
        { local_type = bbb
        ; starts = { day = Julian 60; time = 0 }
        ; ends = { day = Zero_based 300; time = 3600 }
        }
  }
;;

let test_zone_source_rules () =
  let source = Z.create () in
  let rule_zone rule : Z.zone_data =
    { initial = rule.Z.standard; transitions = []; after = Rule rule }
  in
  Z.add_zone source "Test/Us" (rule_zone us_rule);
  Z.add_zone source "Test/Days" (rule_zone day_rule);
  let kind name c = (resolve ~source name c).kind in
  Alcotest.(check bool) "us gap" true (kind "Test/Us" (civil 2030 3 10 2 30) = Gap);
  Alcotest.(check bool) "us fold" true (kind "Test/Us" (civil 2030 11 3 1 30) = Fold);
  Alcotest.(check bool) "us unique" true (kind "Test/Us" (civil 2030 7 1 12 0) = Unique);
  Alcotest.(check bool) "julian gap" true (kind "Test/Days" (civil 2030 3 1 0 30) = Gap);
  Alcotest.(check bool)
    "zero based fold"
    true
    (kind "Test/Days" (civil 2030 10 28 0 30) = Fold);
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

let test_zone_source_loader () =
  let source = Z.create () in
  let loaded = ref [] in
  Z.declare source [ "Test/Lazy"; "Test/Broken"; "Test/Empty" ];
  Z.set_loader source (fun s name ->
    loaded := name :: !loaded;
    match name with
    | "Test/Lazy" -> Z.add_zone s name shift_data
    | "Test/Broken" -> failwith "no data for Test/Broken"
    | _ -> ());
  Alcotest.(check (list string)) "nothing loaded yet" [] !loaded;
  check_shift source "Test/Lazy";
  Alcotest.(check bool) "loaded" true (List.mem "Test/Lazy" !loaded);
  Alcotest.(check bool) "undeclared is unknown" true (info ~source "Test/Other" = None);
  expect_usage_error_now "supplied no data" (fun () -> info ~source "Test/Empty");
  expect_usage_error_now "no data for Test/Broken" (fun () -> info ~source "Test/Broken");
  let source = Z.create () in
  Z.declare source [ "Test/Lazy" ];
  Z.set_loader source (fun s name -> Z.add_zone s name shift_data);
  run ~test_cases:20 (fun tc ->
    let z = Hegel.draw tc (zoned_datetimes ~zones:(zones ~source ()) ()) in
    check_string "lazy zone" "Test/Lazy" z.zone.name)
;;

let test_zone_source_errors () =
  let bundled = Z.bundled () in
  check_string "bundled version" (tzdb_version ()) (Z.version bundled);
  Alcotest.(check bool) "release" true (String.length (tzdb_version ()) >= 5);
  expect_usage_error_now "" (fun () -> Z.add_link bundled "Test/X" ~target:"UTC");
  let source = Z.create () in
  expect_usage_error_now "" (fun () -> Z.add_tzif source "Test/Bad" "not a tzif file");
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
    resolve ~source "Test/Missing" (civil 2000 1 1 0 0));
  expect_usage_error_now "" (fun () ->
    offset_at ~source "Test/Missing" { second = 0; nanosecond = 0 })
;;

let test_queries () =
  let z = resolve new_york (civil 2024 3 10 2 30) in
  Alcotest.(check bool) "gap" true (z.kind = Gap);
  Alcotest.(check bool) "before" true (z.offset_before = Known (-18_000));
  Alcotest.(check bool) "after" true (z.offset_after = Known (-14_400));
  let z = resolve new_york (civil 2024 11 3 1 30) in
  Alcotest.(check bool) "fold" true (z.kind = Fold);
  Alcotest.(check bool) "earlier" false z.fold;
  Alcotest.(check bool) "earlier offset" true (z.offset = Known (-14_400));
  let z = resolve "Asia/Calcutta" (civil 2024 1 1 0 0) in
  check_string "link primary" "Asia/Kolkata" z.zone.primary_name;
  let o = offset_at new_york { second = 1_720_000_000; nanosecond = 0 } in
  check_string "summer" "EDT" o.abbreviation;
  Alcotest.(check bool) "summer offset" true (o.offset = Known (-14_400));
  (match info "UTC" with
   | Some i -> Alcotest.(check int) "utc rank" 0 i.rank
   | None -> Alcotest.fail "UTC unknown");
  match info unknown_zone with
  | Some i -> assert (List.mem Undefined i.tags)
  | None -> Alcotest.fail "unknown zone unknown"
;;

let test_default_zone_source () =
  check_string "bundled default" (tzdb_version ()) (Z.version (default_zone_source ()));
  let source = Z.create () in
  Z.add_zone source "Test/Only" shift_data;
  Z.set_version source "custom";
  let built_before = zones () in
  let fallback =
    Hegel.with_printer
      sexp_of_time_zone
      (Hegel.map (fun name -> { name; primary_name = name }) (Hegel.just "Test/Only"))
  in
  let zoned_before =
    zoned_datetimes
      ~zones:fallback
      ~min_value:(civil 2001 9 9 2 0)
      ~max_value:(civil 2001 9 9 2 0)
      ()
  in
  let opaque_kind = ref None in
  run ~test_cases:5 (fun tc ->
    ignore (Hegel.draw tc built_before : time_zone);
    opaque_kind := Some (Hegel.draw tc zoned_before).offset);
  Alcotest.(check bool) "opaque under bundled" true (!opaque_kind = Some Unknown);
  Fun.protect
    ~finally:(fun () -> set_default_zone_source (Z.bundled ()))
    (fun () ->
       set_default_zone_source source;
       check_string "custom default" "custom" (Z.version (default_zone_source ()));
       Alcotest.(check bool) "query default" true (info "Test/Only" <> None);
       check_shift (default_zone_source ()) "Test/Only";
       run ~test_cases:20 (fun tc ->
         check_string
           "draws custom"
           "Test/Only"
           (Hegel.draw tc (zones ()) : time_zone).name;
         check_string
           "built before follows the default"
           "Test/Only"
           (Hegel.draw tc built_before).name;
         let z = Hegel.draw tc zoned_before in
         Alcotest.(check bool) "fallback uses default" true (z.kind = Gap)));
  check_string "restored" (tzdb_version ()) (Z.version (default_zone_source ()))
;;

let test_specs_freed_with_generator () =
  let draw_once () =
    let gen = spans ~days:(0, 3) () in
    run ~test_cases:3 (fun tc -> ignore (Hegel.draw tc gen : span))
  in
  draw_once ();
  Gc.full_major ();
  Gc.full_major ();
  draw_once ()
;;

let tests =
  [ Alcotest.test_case "default zone source" `Quick test_default_zone_source
  ; Alcotest.test_case "specs freed with generator" `Quick test_specs_freed_with_generator
  ; Alcotest.test_case "date printer" `Quick test_date_printer
  ; Alcotest.test_case "time printer" `Quick test_time_printer
  ; Alcotest.test_case "instant printer" `Quick test_instant_printer
  ; Alcotest.test_case
      "instant printer matches core"
      `Quick
      test_instant_printer_matches_core
  ; Alcotest.test_case "offset printer" `Quick test_offset_printer
  ; Alcotest.test_case "zoned printer" `Quick test_zoned_printer
  ; Alcotest.test_case "duration printer" `Quick test_duration_printer
  ; Alcotest.test_case "span printer" `Quick test_span_printer
  ; Alcotest.test_case "dates" `Quick test_dates
  ; Alcotest.test_case "dates invalid" `Quick test_dates_invalid
  ; Alcotest.test_case "times" `Quick test_times
  ; Alcotest.test_case "datetimes" `Quick test_datetimes
  ; Alcotest.test_case "instants" `Quick test_instants
  ; Alcotest.test_case "offsets" `Quick test_offsets
  ; Alcotest.test_case "durations" `Quick test_durations
  ; Alcotest.test_case "spans" `Quick test_spans
  ; Alcotest.test_case "zones" `Quick test_zones
  ; Alcotest.test_case "zoned datetimes" `Quick test_zoned_datetimes
  ; Alcotest.test_case
      "zoned datetimes any zone generator"
      `Quick
      test_zoned_datetimes_any_zone_generator
  ; Alcotest.test_case "zone source data" `Quick test_zone_source_data
  ; Alcotest.test_case "zone source rules" `Quick test_zone_source_rules
  ; Alcotest.test_case "zone source loader" `Quick test_zone_source_loader
  ; Alcotest.test_case "zone source errors" `Quick test_zone_source_errors
  ; Alcotest.test_case "queries" `Quick test_queries
  ]
;;
