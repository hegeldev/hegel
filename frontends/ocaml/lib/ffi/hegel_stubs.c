#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include <caml/alloc.h>
#include <caml/callback.h>
#include <caml/fail.h>
#include <caml/memory.h>
#include <caml/mlvalues.h>
#include <caml/signals.h>

/* A stub that passes a pointer into the OCaml heap to libhegel (such as
   String_val in hegel_ocaml_mark_complete) must be [@@noalloc] and must not
   release the runtime lock, so the GC cannot move or free that memory while
   libhegel reads it. A stub that releases the lock (such as
   hegel_ocaml_next_test_case) must first copy everything it passes into C
   memory, and must not touch an OCaml value until it has the lock back. */

typedef struct {
  const char *data;
  size_t len;
} hegel_string_t;

typedef int (*generate_integer_fn)(void *, void *, int64_t, int64_t, int64_t *);
typedef int (*generate_boolean_fn)(void *, void *, double, bool, bool, bool *);
typedef int (*generate_float_fn)(void *, void *, uint32_t, double, double, bool, bool,
                                 bool, bool, double, double *);
typedef int (*start_span_fn)(void *, void *, uint64_t);
typedef int (*stop_span_fn)(void *, void *, bool);
typedef int (*new_collection_fn)(void *, void *, uint64_t, uint64_t, void **);
typedef int (*collection_more_fn)(void *, void *, void *, bool *);
typedef int (*call_handle_fn)(void *, void *);
typedef int (*next_test_case_fn)(void *, void *, void **);
typedef int (*test_case_flag_fn)(void *, void *, bool *);
typedef int (*mark_complete_fn)(void *, void *, uint32_t, const hegel_string_t *,
                                const hegel_string_t *, const hegel_string_t *);
typedef int (*next_group_fn)(void *, void *, void *, int64_t *, int64_t *);
typedef int (*next_rule_fn)(void *, void *, void *, int64_t, int64_t *);
typedef int (*rule_rejected_fn)(void *, void *, void *, int64_t);
typedef int (*should_check_invariant_fn)(void *, void *, void *, int64_t, bool *);
typedef int (*generate_string_fn)(void *, void *, void *, hegel_string_t *);
typedef int (*owned_string_free_fn)(void *, hegel_string_t *);

#define HEGEL_E_INTERNAL (-8)
#define STATE_MACHINE_DONE (-1000)

#define PTR(x) ((void *)(x))

static void raise_status(int rc) {
  caml_raise_with_arg(*caml_named_value("hegel_ffi_status"), Val_int(rc));
}

value hegel_ocaml_generate_integer(intnat fn, intnat ctx, intnat tc, intnat lo,
                                   intnat hi) {
  int64_t out = 0;
  int rc = ((generate_integer_fn)fn)(PTR(ctx), PTR(tc), lo, hi, &out);
  if (rc != 0) raise_status(rc);
  return Val_long(out);
}

value hegel_ocaml_generate_integer_byte(value fn, value ctx, value tc, value lo,
                                        value hi) {
  return hegel_ocaml_generate_integer(Nativeint_val(fn), Nativeint_val(ctx),
                                      Nativeint_val(tc), Long_val(lo), Long_val(hi));
}

intnat hegel_ocaml_generate_boolean(intnat fn, intnat ctx, intnat tc, double p,
                                    intnat forced) {
  bool out = false;
  int rc = ((generate_boolean_fn)fn)(PTR(ctx), PTR(tc), p, forced == 1, forced >= 0,
                                     &out);
  return rc != 0 ? rc : out;
}

value hegel_ocaml_generate_string(intnat fn, intnat free_fn, intnat ctx, intnat tc,
                                  intnat generator) {
  hegel_string_t s = {NULL, 0};
  int rc = ((generate_string_fn)fn)(PTR(ctx), PTR(tc), PTR(generator), &s);
  if (rc != 0) raise_status(rc);
  value out = caml_alloc_initialized_string(s.len, s.data);
  ((owned_string_free_fn)free_fn)(PTR(ctx), &s);
  return out;
}

value hegel_ocaml_generate_string_byte(value fn, value free_fn, value ctx, value tc,
                                       value generator) {
  return hegel_ocaml_generate_string(Nativeint_val(fn), Nativeint_val(free_fn),
                                     Nativeint_val(ctx), Nativeint_val(tc),
                                     Nativeint_val(generator));
}

value hegel_ocaml_generate_boolean_byte(value fn, value ctx, value tc, value p,
                                        value forced) {
  return Val_long(hegel_ocaml_generate_boolean(Nativeint_val(fn), Nativeint_val(ctx),
                                               Nativeint_val(tc), Double_val(p),
                                               Long_val(forced)));
}

double hegel_ocaml_generate_float(intnat fn, intnat ctx, intnat tc, double lo,
                                  double hi, intnat flags, double smallest) {
  double out = 0.0;
  int rc = ((generate_float_fn)fn)(PTR(ctx), PTR(tc), 64, lo, hi, flags & 1,
                                   (flags >> 1) & 1, (flags >> 2) & 1,
                                   (flags >> 3) & 1, smallest, &out);
  if (rc != 0) raise_status(rc);
  return out;
}

value hegel_ocaml_generate_float_byte(value *argv, int argn) {
  (void)argn;
  return caml_copy_double(hegel_ocaml_generate_float(
      Nativeint_val(argv[0]), Nativeint_val(argv[1]), Nativeint_val(argv[2]),
      Double_val(argv[3]), Double_val(argv[4]), Long_val(argv[5]),
      Double_val(argv[6])));
}

intnat hegel_ocaml_start_span(intnat fn, intnat ctx, intnat tc, int64_t label) {
  return ((start_span_fn)fn)(PTR(ctx), PTR(tc), (uint64_t)label);
}

value hegel_ocaml_start_span_byte(value fn, value ctx, value tc, value label) {
  return Val_long(hegel_ocaml_start_span(Nativeint_val(fn), Nativeint_val(ctx),
                                         Nativeint_val(tc), Int64_val(label)));
}

intnat hegel_ocaml_stop_span(intnat fn, intnat ctx, intnat tc, value discard) {
  return ((stop_span_fn)fn)(PTR(ctx), PTR(tc), Bool_val(discard));
}

value hegel_ocaml_stop_span_byte(value fn, value ctx, value tc, value discard) {
  return Val_long(hegel_ocaml_stop_span(Nativeint_val(fn), Nativeint_val(ctx),
                                        Nativeint_val(tc), discard));
}

intnat hegel_ocaml_new_collection(intnat fn, intnat ctx, intnat tc, int64_t min_size,
                                  int64_t max_size) {
  void *out = NULL;
  int rc = ((new_collection_fn)fn)(PTR(ctx), PTR(tc), (uint64_t)min_size,
                                   (uint64_t)max_size, &out);
  return rc != 0 ? rc : (intnat)out;
}

value hegel_ocaml_new_collection_byte(value fn, value ctx, value tc, value min_size,
                                      value max_size) {
  return caml_copy_nativeint(hegel_ocaml_new_collection(
      Nativeint_val(fn), Nativeint_val(ctx), Nativeint_val(tc), Int64_val(min_size),
      Int64_val(max_size)));
}

intnat hegel_ocaml_collection_more(intnat fn, intnat ctx, intnat tc, intnat coll) {
  bool out = false;
  int rc = ((collection_more_fn)fn)(PTR(ctx), PTR(tc), PTR(coll), &out);
  return rc != 0 ? rc : out;
}

value hegel_ocaml_collection_more_byte(value fn, value ctx, value tc, value coll) {
  return Val_long(hegel_ocaml_collection_more(Nativeint_val(fn), Nativeint_val(ctx),
                                              Nativeint_val(tc), Nativeint_val(coll)));
}

intnat hegel_ocaml_call_handle(intnat fn, intnat ctx, intnat handle) {
  return ((call_handle_fn)fn)(PTR(ctx), PTR(handle));
}

value hegel_ocaml_call_handle_byte(value fn, value ctx, value handle) {
  return Val_long(hegel_ocaml_call_handle(Nativeint_val(fn), Nativeint_val(ctx),
                                          Nativeint_val(handle)));
}

/* hegel_next_test_case runs the engine on the calling thread and can take a
   while, so the OCaml runtime lock is released for its duration to let other
   OCaml threads run. */
intnat hegel_ocaml_next_test_case(intnat fn, intnat ctx, intnat run) {
  void *out = NULL;
  caml_enter_blocking_section();
  int rc = ((next_test_case_fn)fn)(PTR(ctx), PTR(run), &out);
  caml_leave_blocking_section();
  return rc != 0 ? rc : (intnat)out;
}

value hegel_ocaml_next_test_case_byte(value fn, value ctx, value run) {
  return caml_copy_nativeint(hegel_ocaml_next_test_case(
      Nativeint_val(fn), Nativeint_val(ctx), Nativeint_val(run)));
}

intnat hegel_ocaml_test_case_flag(intnat fn, intnat ctx, intnat tc) {
  bool out = false;
  int rc = ((test_case_flag_fn)fn)(PTR(ctx), PTR(tc), &out);
  return rc != 0 ? rc : out;
}

value hegel_ocaml_test_case_flag_byte(value fn, value ctx, value tc) {
  return Val_long(hegel_ocaml_test_case_flag(Nativeint_val(fn), Nativeint_val(ctx),
                                             Nativeint_val(tc)));
}

static const hegel_string_t *string_opt(value opt, hegel_string_t *s) {
  if (Is_long(opt)) return NULL;
  value str = Field(opt, 0);
  s->data = String_val(str);
  s->len = caml_string_length(str);
  return s;
}

intnat hegel_ocaml_mark_complete(intnat fn, intnat ctx, intnat tc, intnat status,
                                 value origin, value summary, value detail) {
  hegel_string_t o, s, d;
  return ((mark_complete_fn)fn)(PTR(ctx), PTR(tc), (uint32_t)status,
                                string_opt(origin, &o), string_opt(summary, &s),
                                string_opt(detail, &d));
}

value hegel_ocaml_mark_complete_byte(value *argv, int argn) {
  (void)argn;
  return Val_long(hegel_ocaml_mark_complete(
      Nativeint_val(argv[0]), Nativeint_val(argv[1]), Nativeint_val(argv[2]),
      Long_val(argv[3]), argv[4], argv[5], argv[6]));
}

/* Returns the group in the low 32 bits and the round's concurrency level in
   the bits above, STATE_MACHINE_DONE once the machine is done, or a negative
   libhegel return code. */
intnat hegel_ocaml_state_machine_next_group(intnat fn, intnat ctx, intnat tc,
                                            intnat machine) {
  int64_t group = 0;
  int64_t concurrency = 0;
  int rc = ((next_group_fn)fn)(PTR(ctx), PTR(tc), PTR(machine), &group, &concurrency);
  if (rc != 0) return rc;
  if (group == INT64_MIN) return STATE_MACHINE_DONE;
  if (group < 0 || group > UINT32_MAX || concurrency < 0 || concurrency >= (1 << 30))
    return HEGEL_E_INTERNAL;
  return (intnat)((concurrency << 32) | group);
}

value hegel_ocaml_state_machine_next_group_byte(value fn, value ctx, value tc,
                                                value machine) {
  return Val_long(hegel_ocaml_state_machine_next_group(
      Nativeint_val(fn), Nativeint_val(ctx), Nativeint_val(tc),
      Nativeint_val(machine)));
}

intnat hegel_ocaml_state_machine_next_rule(intnat fn, intnat ctx, intnat tc,
                                           intnat machine, intnat worker) {
  int64_t rule = 0;
  int rc = ((next_rule_fn)fn)(PTR(ctx), PTR(tc), PTR(machine), worker, &rule);
  if (rc != 0) return rc;
  if (rule == INT64_MIN) return STATE_MACHINE_DONE;
  if (rule < 0 || rule > UINT32_MAX) return HEGEL_E_INTERNAL;
  return (intnat)rule;
}

value hegel_ocaml_state_machine_next_rule_byte(value fn, value ctx, value tc,
                                               value machine, value worker) {
  return Val_long(hegel_ocaml_state_machine_next_rule(
      Nativeint_val(fn), Nativeint_val(ctx), Nativeint_val(tc),
      Nativeint_val(machine), Long_val(worker)));
}

intnat hegel_ocaml_state_machine_rule_rejected(intnat fn, intnat ctx, intnat tc,
                                               intnat machine, intnat worker) {
  return ((rule_rejected_fn)fn)(PTR(ctx), PTR(tc), PTR(machine), worker);
}

value hegel_ocaml_state_machine_rule_rejected_byte(value fn, value ctx, value tc,
                                                   value machine, value worker) {
  return Val_long(hegel_ocaml_state_machine_rule_rejected(
      Nativeint_val(fn), Nativeint_val(ctx), Nativeint_val(tc),
      Nativeint_val(machine), Long_val(worker)));
}

intnat hegel_ocaml_state_machine_should_check_invariant(intnat fn, intnat ctx,
                                                        intnat tc, intnat machine,
                                                        intnat index) {
  bool out = false;
  int rc = ((should_check_invariant_fn)fn)(PTR(ctx), PTR(tc), PTR(machine), index,
                                           &out);
  return rc != 0 ? rc : out;
}

value hegel_ocaml_state_machine_should_check_invariant_byte(value fn, value ctx,
                                                            value tc, value machine,
                                                            value index) {
  return Val_long(hegel_ocaml_state_machine_should_check_invariant(
      Nativeint_val(fn), Nativeint_val(ctx), Nativeint_val(tc),
      Nativeint_val(machine), Long_val(index)));
}

/* Temporal draws. The bounds are OCaml records of ints (Temporal_types),
   read into C structs before libhegel is called; the result is allocated as
   the matching record after it returns. A zone draw can call a zone loader
   written in OCaml, so no OCaml value is held across a libhegel call. */

typedef struct {
  int32_t year;
  uint8_t month;
  uint8_t day;
} hegel_date_t;

typedef struct {
  uint8_t hour;
  uint8_t minute;
  uint8_t second;
  uint32_t nanosecond;
} hegel_time_t;

typedef struct {
  hegel_date_t date;
  hegel_time_t time;
} hegel_datetime_t;

typedef struct {
  int64_t second;
  uint32_t nanosecond;
} hegel_timestamp_t;

typedef struct {
  int32_t seconds;
  bool known;
} hegel_offset_t;

typedef struct {
  int64_t seconds;
  int32_t nanoseconds;
} hegel_duration_t;

#define SPAN_UNITS 10

typedef struct {
  int64_t units[SPAN_UNITS];
} hegel_span_t;

typedef struct {
  hegel_datetime_t civil;
  int32_t offset_seconds;
  bool offset_known;
  bool fold;
  uint32_t kind;
  int32_t offset_before;
  int32_t offset_after;
  hegel_timestamp_t timestamp;
} hegel_zoned_datetime_t;

typedef int (*generate_date_fn)(void *, void *, hegel_date_t, hegel_date_t, void *,
                                hegel_date_t *);
typedef int (*generate_time_fn)(void *, void *, hegel_time_t, hegel_time_t, uint32_t,
                                hegel_time_t *);
typedef int (*generate_datetime_fn)(void *, void *, hegel_datetime_t, hegel_datetime_t,
                                    uint32_t, hegel_datetime_t *);
typedef int (*generate_timestamp_fn)(void *, void *, hegel_timestamp_t, hegel_timestamp_t,
                                     uint32_t, hegel_timestamp_t *);
typedef int (*generate_offset_fn)(void *, void *, int32_t, int32_t, uint32_t, bool,
                                  hegel_offset_t *);
typedef int (*generate_duration_fn)(void *, void *, hegel_duration_t, hegel_duration_t,
                                    uint32_t, hegel_duration_t *);
typedef int (*generate_span_fn)(void *, void *, void *, hegel_span_t *);
typedef int (*generate_zoned_fn)(void *, void *, void *, void *, hegel_zoned_datetime_t *,
                                 hegel_string_t *);
typedef int (*generate_zoned_for_zone_fn)(void *, void *, void *, hegel_string_t, void *,
                                          hegel_zoned_datetime_t *);

static hegel_date_t date_of_value(value v) {
  hegel_date_t d = {(int32_t)Long_val(Field(v, 0)), (uint8_t)Long_val(Field(v, 1)),
                    (uint8_t)Long_val(Field(v, 2))};
  return d;
}

static hegel_time_t time_of_value(value v) {
  hegel_time_t t = {(uint8_t)Long_val(Field(v, 0)), (uint8_t)Long_val(Field(v, 1)),
                    (uint8_t)Long_val(Field(v, 2)), (uint32_t)Long_val(Field(v, 3))};
  return t;
}

static hegel_datetime_t datetime_of_value(value v) {
  hegel_datetime_t dt = {date_of_value(Field(v, 0)), time_of_value(Field(v, 1))};
  return dt;
}

static hegel_timestamp_t timestamp_of_value(value v) {
  hegel_timestamp_t t = {Long_val(Field(v, 0)), (uint32_t)Long_val(Field(v, 1))};
  return t;
}

static hegel_duration_t duration_of_value(value v) {
  hegel_duration_t d = {Long_val(Field(v, 0)), (int32_t)Long_val(Field(v, 1))};
  return d;
}

static value alloc_date(hegel_date_t d) {
  value v = caml_alloc_small(3, 0);
  Field(v, 0) = Val_long(d.year);
  Field(v, 1) = Val_long(d.month);
  Field(v, 2) = Val_long(d.day);
  return v;
}

static value alloc_time(hegel_time_t t) {
  value v = caml_alloc_small(4, 0);
  Field(v, 0) = Val_long(t.hour);
  Field(v, 1) = Val_long(t.minute);
  Field(v, 2) = Val_long(t.second);
  Field(v, 3) = Val_long(t.nanosecond);
  return v;
}

static value alloc_datetime(hegel_datetime_t dt) {
  CAMLparam0();
  CAMLlocal3(date, time, v);
  date = alloc_date(dt.date);
  time = alloc_time(dt.time);
  v = caml_alloc_small(2, 0);
  Field(v, 0) = date;
  Field(v, 1) = time;
  CAMLreturn(v);
}

static value alloc_pair_of_ints(intnat a, intnat b) {
  value v = caml_alloc_small(2, 0);
  Field(v, 0) = Val_long(a);
  Field(v, 1) = Val_long(b);
  return v;
}

/* Temporal_types.Zoned.t. */
static value alloc_zoned(const hegel_zoned_datetime_t *z) {
  CAMLparam0();
  CAMLlocal3(civil, timestamp, v);
  civil = alloc_datetime(z->civil);
  timestamp = alloc_pair_of_ints(z->timestamp.second, z->timestamp.nanosecond);
  v = caml_alloc_small(8, 0);
  Field(v, 0) = civil;
  Field(v, 1) = Val_long(z->offset_seconds);
  Field(v, 2) = Val_bool(z->offset_known);
  Field(v, 3) = Val_bool(z->fold);
  Field(v, 4) = Val_long(z->kind);
  Field(v, 5) = Val_long(z->offset_before);
  Field(v, 6) = Val_long(z->offset_after);
  Field(v, 7) = timestamp;
  CAMLreturn(v);
}

value hegel_ocaml_generate_date(intnat fn, intnat ctx, intnat tc, value min, value max,
                                intnat options) {
  hegel_date_t lo = date_of_value(min), hi = date_of_value(max), out;
  int rc = ((generate_date_fn)fn)(PTR(ctx), PTR(tc), lo, hi, PTR(options), &out);
  if (rc != 0) raise_status(rc);
  return alloc_date(out);
}

value hegel_ocaml_generate_date_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_date(Nativeint_val(argv[0]), Nativeint_val(argv[1]),
                                   Nativeint_val(argv[2]), argv[3], argv[4],
                                   Nativeint_val(argv[5]));
}

value hegel_ocaml_generate_time(intnat fn, intnat ctx, intnat tc, value min, value max,
                                intnat precision) {
  hegel_time_t lo = time_of_value(min), hi = time_of_value(max), out;
  int rc = ((generate_time_fn)fn)(PTR(ctx), PTR(tc), lo, hi, (uint32_t)precision, &out);
  if (rc != 0) raise_status(rc);
  return alloc_time(out);
}

value hegel_ocaml_generate_time_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_time(Nativeint_val(argv[0]), Nativeint_val(argv[1]),
                                   Nativeint_val(argv[2]), argv[3], argv[4],
                                   Long_val(argv[5]));
}

value hegel_ocaml_generate_datetime(intnat fn, intnat ctx, intnat tc, value min,
                                    value max, intnat precision) {
  hegel_datetime_t lo = datetime_of_value(min), hi = datetime_of_value(max), out;
  int rc =
      ((generate_datetime_fn)fn)(PTR(ctx), PTR(tc), lo, hi, (uint32_t)precision, &out);
  if (rc != 0) raise_status(rc);
  return alloc_datetime(out);
}

value hegel_ocaml_generate_datetime_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_datetime(Nativeint_val(argv[0]), Nativeint_val(argv[1]),
                                       Nativeint_val(argv[2]), argv[3], argv[4],
                                       Long_val(argv[5]));
}

value hegel_ocaml_generate_timestamp(intnat fn, intnat ctx, intnat tc, value min,
                                     value max, intnat precision) {
  hegel_timestamp_t lo = timestamp_of_value(min), hi = timestamp_of_value(max), out;
  int rc =
      ((generate_timestamp_fn)fn)(PTR(ctx), PTR(tc), lo, hi, (uint32_t)precision, &out);
  if (rc != 0) raise_status(rc);
  return alloc_pair_of_ints(out.second, out.nanosecond);
}

value hegel_ocaml_generate_timestamp_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_timestamp(Nativeint_val(argv[0]), Nativeint_val(argv[1]),
                                        Nativeint_val(argv[2]), argv[3], argv[4],
                                        Long_val(argv[5]));
}

value hegel_ocaml_generate_duration(intnat fn, intnat ctx, intnat tc, value min,
                                    value max, intnat precision) {
  hegel_duration_t lo = duration_of_value(min), hi = duration_of_value(max), out;
  int rc =
      ((generate_duration_fn)fn)(PTR(ctx), PTR(tc), lo, hi, (uint32_t)precision, &out);
  if (rc != 0) raise_status(rc);
  return alloc_pair_of_ints(out.seconds, out.nanoseconds);
}

value hegel_ocaml_generate_duration_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_duration(Nativeint_val(argv[0]), Nativeint_val(argv[1]),
                                       Nativeint_val(argv[2]), argv[3], argv[4],
                                       Long_val(argv[5]));
}

/* Returns Temporal_types.Offset.t. */
value hegel_ocaml_generate_offset(intnat fn, intnat ctx, intnat tc, intnat lo, intnat hi,
                                  intnat granularity, intnat allow_unknown) {
  hegel_offset_t out = {0, false};
  int rc = ((generate_offset_fn)fn)(PTR(ctx), PTR(tc), (int32_t)lo, (int32_t)hi,
                                    (uint32_t)granularity, allow_unknown != 0, &out);
  if (rc != 0) raise_status(rc);
  value v = caml_alloc_small(2, 0);
  Field(v, 0) = Val_long(out.seconds);
  Field(v, 1) = Val_bool(out.known);
  return v;
}

value hegel_ocaml_generate_offset_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_offset(Nativeint_val(argv[0]), Nativeint_val(argv[1]),
                                     Nativeint_val(argv[2]), Long_val(argv[3]),
                                     Long_val(argv[4]), Long_val(argv[5]),
                                     Long_val(argv[6]));
}

value hegel_ocaml_generate_span(intnat fn, intnat ctx, intnat tc, intnat spec) {
  hegel_span_t out;
  int rc = ((generate_span_fn)fn)(PTR(ctx), PTR(tc), PTR(spec), &out);
  if (rc != 0) raise_status(rc);
  value v = caml_alloc_small(SPAN_UNITS, 0);
  for (int i = 0; i < SPAN_UNITS; i++) Field(v, i) = Val_long(out.units[i]);
  return v;
}

value hegel_ocaml_generate_span_byte(value fn, value ctx, value tc, value spec) {
  return hegel_ocaml_generate_span(Nativeint_val(fn), Nativeint_val(ctx),
                                   Nativeint_val(tc), Nativeint_val(spec));
}

/* Returns the drawn value and its zone name as a pair. */
value hegel_ocaml_generate_zoned(intnat fn, intnat free_fn, intnat ctx, intnat tc,
                                 intnat zones, intnat spec) {
  CAMLparam0();
  CAMLlocal3(zone, zoned, pair);
  hegel_zoned_datetime_t out;
  hegel_string_t name = {NULL, 0};
  int rc =
      ((generate_zoned_fn)fn)(PTR(ctx), PTR(tc), PTR(zones), PTR(spec), &out, &name);
  if (rc != 0) raise_status(rc);
  zone = caml_alloc_initialized_string(name.len, name.data);
  ((owned_string_free_fn)free_fn)(PTR(ctx), &name);
  zoned = alloc_zoned(&out);
  pair = caml_alloc_small(2, 0);
  Field(pair, 0) = zoned;
  Field(pair, 1) = zone;
  CAMLreturn(pair);
}

value hegel_ocaml_generate_zoned_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_zoned(Nativeint_val(argv[0]), Nativeint_val(argv[1]),
                                    Nativeint_val(argv[2]), Nativeint_val(argv[3]),
                                    Nativeint_val(argv[4]), Nativeint_val(argv[5]));
}

/* The zone name is copied out of the OCaml heap first: a zone loader written
   in OCaml can run during the call and move it. */
value hegel_ocaml_generate_zoned_for_zone(intnat fn, intnat ctx, intnat tc,
                                          intnat source, value name, intnat spec) {
  hegel_zoned_datetime_t out;
  size_t len = caml_string_length(name);
  char *copy = caml_stat_alloc(len + 1);
  memcpy(copy, String_val(name), len);
  copy[len] = '\0';
  hegel_string_t zone = {copy, len};
  int rc = ((generate_zoned_for_zone_fn)fn)(PTR(ctx), PTR(tc), PTR(source), zone,
                                            PTR(spec), &out);
  caml_stat_free(copy);
  if (rc != 0) raise_status(rc);
  return alloc_zoned(&out);
}

value hegel_ocaml_generate_zoned_for_zone_byte(value *argv, int argc) {
  (void)argc;
  return hegel_ocaml_generate_zoned_for_zone(
      Nativeint_val(argv[0]), Nativeint_val(argv[1]), Nativeint_val(argv[2]),
      Nativeint_val(argv[3]), argv[4], Nativeint_val(argv[5]));
}
