# Run tests on Antithesis

[Antithesis](https://antithesis.com) runs software in a deterministic
simulation and explores how it behaves, with a fuzzer controlling every source
of randomness. Hegel tests can run inside Antithesis as *workloads*: Antithesis
controls every value the test draws, and explores them far more deeply than a
normal test run can.

Hegel detects that it is running inside Antithesis (from the
`ANTITHESIS_OUTPUT_DIR` environment variable) and uses the `workload` settings
profile there. Compared with running locally, the `workload` profile:

- uses the `urandom` {class}`~hegel.Backend`, which takes every choice from the
  Antithesis fuzzer rather than from Hegel's own random number generator;
- turns off the example database;
- suppresses every health check, since Antithesis pauses and resumes the
  system under test, which would trip health checks based on time.

Each test's verdict, whether it passed or failed, is reported to Antithesis as
an assertion named after the test, so failures appear in the Antithesis
report.

Nothing needs configuring for this. To try a test with the same backend
outside Antithesis, select the profile yourself:

```bash
pytest --hegel-profile=workload
```

Outside Antithesis the `urandom` backend takes fresh entropy from the
operating system (`/dev/urandom` on Unix) for every choice, so runs cannot be
reproduced from a seed.
