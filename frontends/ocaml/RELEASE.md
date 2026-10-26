RELEASE_TYPE: minor

This release removes the `printable`/`unprintable` phantom from generators: `('a, 'p) generator` is now `'a generator`, and `Hegel.printable`, `Hegel.unprintable`, `Generators.printable` and `Generators.unprintable` are gone. To upgrade, drop the second type argument from any annotation, e.g. `(int, printable) generator` becomes `int generator`.

Every generator can now be drawn with `draw` and passed to `one_of`, `optional`, `tuples2`, `tuples3`, `tuples4`, `lists`, `assoc_lists`, `hash_tables` and `Hegel_jane.hash_tables`, including the results of `map`, `flat_map`, `just`, `sampled_from` and `composite`. A value with no printer prints as `<opaque>` in the failing-test output, so `with_printer` is now only needed to print it in full, and `one_of` can mix generators with and without printers.

```ocaml
let%hegel_test pairs tc =
  let doubled = map (fun n -> n * 2) (integers ()) in
  let x, y = draw tc (one_of [ tuples2 doubled (integers ()); tuples2 (integers ()) doubled ]) in
  (* on failure, e.g.: (<opaque> 5) *)
  assert (x <> y)
;;
```
