#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include <caml/alloc.h>
#include <caml/callback.h>
#include <caml/fail.h>
#include <caml/memory.h>
#include <caml/mlvalues.h>
#include <caml/signals.h>

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
typedef int (*handle_free_fn)(void *, void *);
typedef int (*next_test_case_fn)(void *, void *, void **);
typedef int (*test_case_flag_fn)(void *, void *, bool *);
typedef int (*mark_complete_fn)(void *, void *, uint32_t, const hegel_string_t *,
                                const hegel_string_t *, const hegel_string_t *);

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

intnat hegel_ocaml_handle_free(intnat fn, intnat ctx, intnat handle) {
  return ((handle_free_fn)fn)(PTR(ctx), PTR(handle));
}

value hegel_ocaml_handle_free_byte(value fn, value ctx, value handle) {
  return Val_long(hegel_ocaml_handle_free(Nativeint_val(fn), Nativeint_val(ctx),
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
