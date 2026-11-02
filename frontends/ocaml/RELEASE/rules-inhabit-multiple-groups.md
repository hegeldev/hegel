RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups. A rule's groups are now a list, and a rule in several groups runs in the rounds of each of them, so it may overlap the rules of every group it belongs to while those groups' other rules still never overlap each other. A bare `[@@rule]` still indicates the anonymous group.

Before:

```ocaml
let write tc () state = ... [@@rule { group = "write" }]
```

After:

```ocaml
let write tc () state = ... [@@rule { groups = [ "write" ] }]
let flush tc () state = ... [@@rule { groups = [ "flush" ] }]
let read tc () state = ... [@@rule { groups = [ "write"; "flush" ] }]
```

Without the PPX, `Stateful.Concurrent_rule.create` takes `?groups:string list` in place of `?group:string`, and `Stateful.Concurrent_rule.groups` replaces `Stateful.Concurrent_rule.group`:

```ocaml
let read =
  Stateful.Concurrent_rule.create ~name:"read" ~groups:[ "write"; "flush" ] ~step ()
;;

let groups : string list = Stateful.Concurrent_rule.groups read
```

The list must name at least one group and must not repeat one. The engine checks this when the machine starts and reports a violation as `Hegel.Usage_error`. `Hegel_jane_async.Stateful.Concurrent_rule` changes the same way.

The `Stateful` documentation's key-value store example now puts its `read` rule in both the `operations` and the `snapshot` group, so reads overlap snapshots as well as writes.
