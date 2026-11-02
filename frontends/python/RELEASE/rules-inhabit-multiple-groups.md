RELEASE_TYPE: minor

This release lets a rule of a concurrent state machine belong to several concurrency groups. The `group` argument of `@hegel.rule` and `Machine.rule` is replaced by `groups`, a list of group names; a rule runs in the rounds of every group it names, so it may overlap the rules of each of them. `Rule.group` becomes `Rule.groups`, a tuple. Omitting `groups` still puts the rule in the anonymous group.

Before:

```python
@hegel.rule(group="write")
def write(self, tc): ...
```

After:

```python
@hegel.rule(groups=["write"])
def write(self, tc): ...

@hegel.rule(groups=["write", "flush"])
def read(self, tc): ...
```

Here `read` may run concurrently with `write` in one round and with `flush` in another, while `write` and `flush` never overlap each other, which is how a two-tier store whose flush briefly hides a key can be caught by a read that overlaps it.

The list must name at least one group and must not repeat one: an empty list or a repeated group is an `InvalidArgument` reported by the engine when the machine runs.
