RELEASE_TYPE: patch

This patch adds `?pause_faults` to `Stateful.Invariant.create`, and the matching `pause_faults` option of `[@@invariant]`, for an invariant that should observe the system under test with fault injection paused. Every run of such an invariant (on the initial state, after a sampled step or round, and on the final state) is bracketed with the engine's new `hegel_pause_faults` and `hegel_unpause_faults` calls, on sequential, concurrent and Async machines alike, and the unpause runs even when the invariant raises. The option combines with `always_check` in either order.

```ocaml
module%hegel_state_machine Stack = struct
  type state = int list ref

  let push tc stack =
    let n = draw tc (integers ~min_value:0 ~max_value:9 ()) in
    stack := n :: !stack
  [@@rule]

  let consistent _tc stack = assert (List.length !stack >= 0)
  [@@invariant { always_check = true; pause_faults = true }]
end
```

Pausing is currently a no-op: a later engine release will make it pause [Antithesis](https://antithesis.com/) fault injection for the duration of the invariant. `Stateful.Invariant.always_check` and `Stateful.Invariant.pause_faults` read the two flags back.
