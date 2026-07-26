# Engine API

The `engine` Cargo feature is for parser and state-machine implementations that need to preserve graph sharing on hot paths. It is optional; ordinary users only see the semantic weighted-stack API.

```toml
weighted-gss = { version = "0.2", features = ["engine"] }
```

The module does not expose graph nodes, pointer identity, structural statistics, parser actions, or application-specific caches.

## Path-local weights

`path_weights(&gss)` iterates over stored path weights without expanding concrete stacks. `filter_map_path_weights(&gss, f)` transforms or removes those stored weights while preserving the immutable stack DAG.

These operations are intentionally named *path* operations. They act before coincident concrete stacks are necessarily joined, so in general:

```text
f(a ⋁ b) may differ from f(a) ⋁ f(b)
```

Iteration order and weight placement are not semantic guarantees.

## Bounded stack inspection

`for_each_stack_top_first(&gss, limit, visit)` visits distinct concrete stacks as top-first slices. Equal stacks are coalesced and their weights joined before the callback. It returns `StackLimitExceeded` rather than traversing more than `limit` distinct stacks.

The implementation handles the common one-stack case inline and uses memoised bounded collection for shared ambiguous languages. The callback must not retain its borrowed stack slice.

## Linear prefixes

`linear_prefix(&gss)` returns a `LinearPrefix` when the current representation has one homogeneous weight and an accessible linear top prefix. The prefix may sit over a branched hidden floor.

`LinearPrefix` supports:

- `len`, `is_empty`, and `get(depth_from_top)`;
- `floor_is_empty`;
- `push` and `popn`;
- `into_gss`.

A nonzero result from `popn` is the number of requested pops that reached beyond the accessible prefix. Applications can then convert back to a GSS and handle the remainder generally.

## Exact stack-language keys

`StackLanguageInterner::key(&gss)` returns an exact `StackLanguageId` for the unweighted set of concrete stacks. Weights, duplicate representation paths, segment boundaries, and DAG layout do not affect the key.

IDs are meaningful only within one interner. Use one interner for one fixpoint computation.

## Deliberately application-local

The engine module does not provide batched pop/push operations, guarded depth filtering, branch-wrapper types, representation IDs, profiling summaries, or accumulator-specific helpers. Those are concise to express in the application and would make this crate inherit one parser's vocabulary.
