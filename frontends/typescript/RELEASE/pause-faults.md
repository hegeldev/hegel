RELEASE_TYPE: patch

This patch adds a `pauseFaults` option to the invariants of a `stateful.StateMachine`, alongside `alwaysCheck`. An invariant declared with `{ check, pauseFaults: true }` asks the engine to pause fault injection while it runs, so that inside [Antithesis](https://antithesis.com/) no fault lands in the middle of the check; faults resume as soon as the check returns or throws.

```ts
const machine: hegel.stateful.StateMachine<Cluster> = {
  rules: { ... },
  invariants: {
    replicasAgree: {
      pauseFaults: true,
      check: async (_tc, cluster) => {
        expect(await cluster.replicaStates()).toSatisfy(allEqual);
      },
    },
  },
};
```

The option is accepted on every host and is currently a no-op: a later engine release will act on it to pause Antithesis fault injection for the duration of the check.
