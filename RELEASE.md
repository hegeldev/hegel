RELEASE_TYPE: patch

This patch adds `Hegel.main` and the `let%hegel_main` PPX, for creating an executable that runs one test case per invocation, such as a workload in Antithesis.

```ocaml
let%hegel_main workload tc = Counter.run ~step_count:max_int tc ~init:(ref 0)
let () = workload ()
```
