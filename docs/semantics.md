# Semantics and invariants

A `WeightedGss<T, W>` denotes a finitely supported map

```text
bottom-to-top stacks over T  ->  weights in W
```

The graph is a compressed representation of that map. Public operations are defined by their effect on the map, except for methods explicitly documented as structural diagnostics or normalization operations.

## Weight join

When two operations produce the same concrete stack, their weights are combined with `Weight::join`. A valid `Weight` implementation must be:

- associative: `(a ⋁ b) ⋁ c = a ⋁ (b ⋁ c)`;
- commutative: `a ⋁ b = b ⋁ a`; and
- idempotent: `a ⋁ a = a`.

The graph may retain more than one structural route to the same concrete stack. Idempotence makes the denotation independent of those representation choices. A non-idempotent operation such as integer addition is therefore not a valid weight join.

## Empty GSS and empty stack

These are different values:

- `WeightedGss::empty()` denotes the empty map.
- `WeightedGss::from_single_stack(Vec::new(), weight)` denotes a map containing the empty stack.

`is_empty()` distinguishes them. `isolate(None)` keeps only empty-stack entries.

## Push and pop

`push(x)` appends `x` to every represented stack without changing its weight.

`pop()` removes one value from each non-empty stack and discards empty stacks. When several source stacks pop to the same result, their weights join.

`popn(n)` removes exactly `n` values from each entry:

- stacks longer than `n` retain their remaining prefix;
- stacks of length `n` become empty stacks; and
- stacks shorter than `n` are discarded.

`popn(0)` is a no-op. The Rust method accepts an `isize`; non-positive values are treated as a no-op. The Python method rejects negative values with `ValueError`.

## Merge

`merge` forms the pointwise union of two stack-to-weight maps. Entries present in only one input are retained unchanged. Weights for keys present in both inputs are joined.

## Persistence and identity

Operations do not mutate a `WeightedGss`; they return a new value and retain `Arc`-backed sharing where possible. `ptr_eq` and `ptr_key` expose root-allocation identity for memoization. They do not express semantic equality and must not be persisted across processes.

## Materialization and counts

`to_stacks(max_stacks)` is for diagnostics, tests, and bounded interoperability. It returns `None` in Rust, or raises `OverflowError` in Python, rather than truncating when the structural path count exceeds the limit.

`path_count_at_most(limit)` counts structural graph paths, capped at `limit`. Structurally distinct paths can denote the same concrete stack, so this count can exceed the number of distinct keys after canonicalizing `to_stacks`.

`summary()` reports graph structure without materializing all paths.

## Segment backend

The internal deterministic-segment representation is selected once per process with the `STACKVEC` environment variable:

- `vec` (default): ordinary `Vec` storage;
- `arc`: copy-on-write `Arc<Vec<_>>` storage.

The choice affects performance and sharing, not public semantics. CI runs the full Rust suite against both backends.
