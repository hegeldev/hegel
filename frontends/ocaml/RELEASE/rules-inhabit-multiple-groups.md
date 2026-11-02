RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups.

The `group` option of `[@@rule]` is now `groups`, which takes a list or a string for rules belonging to one group.

`Stateful.Concurrent_rule.create` now takes `?groups:string list`. `Stateful.Concurrent_rule.group : 'a Concurrent_rule.t -> string` is now `Stateful.Concurrent_rule.groups : 'a Concurrent_rule.t -> string list`. `Hegel_jane_async.Stateful.Concurrent_rule` has the same changes.

A rule with an empty or repeated group list raises `Hegel.Usage_error`.
