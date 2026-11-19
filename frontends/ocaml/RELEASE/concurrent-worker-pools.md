RELEASE_TYPE: minor

This release makes concurrent state machines reuse a fixed set of workers throughout each run when using `Concurrency.threads`, `Concurrency.domains`, or `Hegel_jane_concurrent.of_concurrent`. Workers are released when the run ends, including when a test fails.

`Concurrency.t` is now abstract. Custom runners use `Concurrency.create ~spawn_join_n` in place of a `{ spawn_join_n }` record. On OxCaml, `Hegel_jane_concurrent.create_local` accepts custom schedulers with a local context.
