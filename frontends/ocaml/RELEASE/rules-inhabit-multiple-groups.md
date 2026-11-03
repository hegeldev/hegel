RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups.

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
