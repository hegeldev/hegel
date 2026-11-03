[@@@alert "-deprecated"]

open Hegel
open Generators

(** Test: dates generates ISO 8601 [YYYY-MM-DD] strings that parse as valid
    calendar dates with year in [1, 9999] (round-tripped through
    [Core.Date.of_string]). *)
let test_dates_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:10 ()) (fun tc ->
    let d = Hegel.draw tc (dates ()) in
    assert (String.length d = 10 && d.[4] = '-' && d.[7] = '-');
    let y = Core.Date.year (Core.Date.of_string d) in
    assert (y >= 1 && y <= 9999))
;;

let time_of_string s =
  Scanf.sscanf s "%2d:%2d:%2d.%9d%!" (fun hour minute second nanosecond ->
    { hour; minute; second; nanosecond })
;;

let date_of_string s =
  Scanf.sscanf s "%4d-%2d-%2d%!" (fun year month day -> { year; month; day })
;;

let within ~lo ~hi x = compare lo x <= 0 && compare x hi <= 0
let first_time = { hour = 0; minute = 0; second = 0; nanosecond = 0 }
let last_time = { hour = 23; minute = 59; second = 59; nanosecond = 999_999_999 }

(** Test: times generates [HH:MM:SS.fffffffff] strings that decode to times
    within the day. *)
let test_times_e2e () =
  let saw_sub_microsecond = ref false in
  Hegel.run_hegel_test
    ~settings:(Hegel.Settings.create ~test_cases:100 ~seed:0 ())
    (fun tc ->
       let s = Hegel.draw tc (times ()) in
       let t = time_of_string s in
       assert (within ~lo:first_time ~hi:last_time t);
       ignore (Core.Time_ns.Ofday.of_string s : Core.Time_ns.Ofday.t);
       if t.nanosecond mod 1000 <> 0 then saw_sub_microsecond := true);
  assert !saw_sub_microsecond
;;

let datetime_of_string s =
  assert (String.length s = 29 && s.[10] = 'T');
  date_of_string (String.sub s 0 10), time_of_string (String.sub s 11 18)
;;

(** Test: times honors inclusive bounds. *)
let test_times_bounds_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:100 ()) (fun tc ->
    let lo = Hegel.draw tc (times ()) in
    let min_time = time_of_string lo in
    let max_time = time_of_string (Hegel.draw tc (times ~min_time ())) in
    let t = time_of_string (Hegel.draw tc (times ~min_time ~max_time ())) in
    assert (within ~lo:min_time ~hi:max_time t);
    let point = Hegel.draw tc (times ~min_time ~max_time:min_time ()) in
    Alcotest.(check string) "point range" lo point)
;;

(** Test: dates honors inclusive bounds. *)
let test_dates_bounds_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:100 ()) (fun tc ->
    let lo = Hegel.draw tc (dates ()) in
    let min_date = date_of_string lo in
    let max_date = date_of_string (Hegel.draw tc (dates ~min_date ())) in
    let d = date_of_string (Hegel.draw tc (dates ~min_date ~max_date ())) in
    assert (within ~lo:min_date ~hi:max_date d);
    let point = Hegel.draw tc (dates ~min_date ~max_date:min_date ()) in
    Alcotest.(check string) "point range" lo point)
;;

(** Test: datetimes generates [YYYY-MM-DDTHH:MM:SS.fffffffff] strings whose date
    and time parts decode to values in the default ranges. *)
let test_datetimes_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:10 ()) (fun tc ->
    let date, time = datetime_of_string (Hegel.draw tc (datetimes ())) in
    assert (date.year >= 1 && date.year <= 9999);
    assert (within ~lo:first_time ~hi:last_time time))
;;

(** Test: datetimes honors inclusive bounds. *)
let test_datetimes_bounds_e2e () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:100 ()) (fun tc ->
    let lo = Hegel.draw tc (datetimes ()) in
    let min_datetime = datetime_of_string lo in
    let max_datetime = datetime_of_string (Hegel.draw tc (datetimes ~min_datetime ())) in
    let dt =
      datetime_of_string (Hegel.draw tc (datetimes ~min_datetime ~max_datetime ()))
    in
    assert (within ~lo:min_datetime ~hi:max_datetime dt);
    let point = Hegel.draw tc (datetimes ~min_datetime ~max_datetime:min_datetime ()) in
    Alcotest.(check string) "point range" lo point)
;;

let expect_usage_error name gen expected =
  match
    Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:20 ()) (fun tc ->
      ignore (Hegel.draw tc gen : string))
  with
  | exception Hegel.Usage_error msg ->
    Alcotest.(check string) (name ^ ": diagnostic") expected msg
  | () -> Alcotest.failf "%s: expected Usage_error" name
;;

(** Test: date bounds the engine rejects (a calendar-invalid day, an
    out-of-range month, crossed bounds) abort the run with [Usage_error]. *)
let test_dates_invalid_bounds () =
  List.iter
    (fun (name, min_date, max_date, expected) ->
       expect_usage_error name (dates ~min_date ~max_date ()) expected)
    [ ( "feb 29 2023"
      , { year = 2023; month = 2; day = 29 }
      , { year = 2024; month = 1; day = 1 }
      , "min_value is not a valid date (year in [-999999, 999999]): 02023-02-29" )
    ; ( "month 13"
      , { year = 2024; month = 1; day = 1 }
      , { year = 2024; month = 13; day = 1 }
      , "max_value is not a valid date (year in [-999999, 999999]): 02024-13-01" )
    ; ( "min > max"
      , { year = 2024; month = 1; day = 2 }
      , { year = 2024; month = 1; day = 1 }
      , "generate_date requires min_value <= max_value, got [Date { year: 2024, month: \
         1, day: 2 }, Date { year: 2024, month: 1, day: 1 }]" )
    ]
;;

(** Test: time bounds the engine rejects (an out-of-range part, crossed bounds)
    abort the run with [Usage_error] carrying the engine's diagnostic. *)
let test_times_invalid_bounds () =
  List.iter
    (fun (name, min_time, max_time, expected) ->
       expect_usage_error name (times ~min_time ~max_time ()) expected)
    [ ( "hour 24"
      , first_time
      , { hour = 24; minute = 0; second = 0; nanosecond = 0 }
      , "max_value is not a valid time: 24:00:00.000000000" )
    ; ( "nanosecond 1e9"
      , first_time
      , { hour = 0; minute = 0; second = 0; nanosecond = 1_000_000_000 }
      , "max_value is not a valid time: 00:00:00.1000000000" )
    ; ( "min > max"
      , { hour = 0; minute = 0; second = 0; nanosecond = 1 }
      , first_time
      , "generate_time requires min_value <= max_value, got [Time { hour: 0, minute: 0, \
         second: 0, nanosecond: 1 }, Time { hour: 0, minute: 0, second: 0, nanosecond: 0 \
         }]" )
    ]
;;

(** Test: datetime bounds ordered by date then time; crossed bounds abort the
    run with [Usage_error] carrying the engine's diagnostic. *)
let test_datetimes_invalid_bounds () =
  expect_usage_error
    "min > max"
    (datetimes
       ~min_datetime:
         ( { year = 2024; month = 1; day = 1 }
         , { hour = 0; minute = 0; second = 0; nanosecond = 1 } )
       ~max_datetime:({ year = 2024; month = 1; day = 1 }, first_time)
       ())
    "generate_datetime requires min_value <= max_value, got [DateTime { date: Date { \
     year: 2024, month: 1, day: 1 }, time: Time { hour: 0, minute: 0, second: 0, \
     nanosecond: 1 } }, DateTime { date: Date { year: 2024, month: 1, day: 1 }, time: \
     Time { hour: 0, minute: 0, second: 0, nanosecond: 0 } }]"
;;

let test_make_builders () =
  Hegel.run_hegel_test ~settings:(Hegel.Settings.create ~test_cases:20 ()) (fun tc ->
    let d =
      Hegel.draw
        tc
        (make_dates
           ~of_date:(fun { year; month; day } ->
             Core.Date.create_exn ~y:year ~m:(Core.Month.of_int_exn month) ~d:day)
           ~sexp_of:Core.Date.sexp_of_t
           ())
    in
    assert (Core.Date.year d >= 1 && Core.Date.year d <= 9999);
    let seconds =
      Hegel.draw
        tc
        (make_times
           ~of_time:(fun { hour; minute; second; _ } ->
             (hour * 3600) + (minute * 60) + second)
           ~sexp_of:Sexplib0.Sexp_conv.sexp_of_int
           ())
    in
    assert (seconds >= 0 && seconds < 86_400);
    let year =
      Hegel.draw
        tc
        (make_datetimes
           ~of_datetime:(fun ((date : date), (_ : time)) -> date.year)
           ~sexp_of:Sexplib0.Sexp_conv.sexp_of_int
           ())
    in
    assert (year >= 1 && year <= 9999))
;;

let tests =
  [ Alcotest.test_case "dates e2e" `Quick test_dates_e2e
  ; Alcotest.test_case "dates bounds e2e" `Quick test_dates_bounds_e2e
  ; Alcotest.test_case "dates invalid bounds" `Quick test_dates_invalid_bounds
  ; Alcotest.test_case "times e2e" `Quick test_times_e2e
  ; Alcotest.test_case "times bounds e2e" `Quick test_times_bounds_e2e
  ; Alcotest.test_case "times invalid bounds" `Quick test_times_invalid_bounds
  ; Alcotest.test_case "datetimes e2e" `Quick test_datetimes_e2e
  ; Alcotest.test_case "datetimes bounds e2e" `Quick test_datetimes_bounds_e2e
  ; Alcotest.test_case "datetimes invalid bounds" `Quick test_datetimes_invalid_bounds
  ; Alcotest.test_case "make builders" `Quick test_make_builders
  ]
;;
