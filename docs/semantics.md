# Semantics and invariants

A `LeveledGSS<T, A>` denotes a finite collection of stack paths. Each path has:

- a bottom-to-top sequence of values of type `T`; and
- an accumulator of type `A`.

The graph is an implementation detail. Public operations are defined by their effect on this denotation, except for methods explicitly described as structural diagnostics or advanced normalization operations.

## Accumulator join

When two operations produce the same concrete stack, their accumulators are combined with `Merge::merge`. `Merge` must be:

- associative: `(a ⋁ b) ⋁ c = a ⋁ (b ⋁ c)`;
- commutative: `a ⋁ b = b ⋁ a`; and
- idempotent: `a ⋁ a = a`.

The graph may retain more than one structural route to the same concrete stack. Idempotence makes the result independent of those representation choices. A non-idempotent operation such as integer addition is therefore not a valid `Merge` implementation.

## Empty GSS and empty stack

These are different states:

- `LeveledGSS::empty()` contains no paths.
- `LeveledGSS::from_single_stack(Vec::new(), acc)` contains one path whose stack is empty.

`is_empty()` distinguishes them. `isolate(None)` keeps only empty-stack paths.

## Push and pop

`push(x)` appends `x` to every active stack.

`pop()` removes one value from each non-empty stack and discards empty-stack paths.

`popn(n)` removes exactly `n` values from each path:

- paths longer than `n` retain their remaining prefix;
- paths of length `n` become empty-stack paths; and
- paths shorter than `n` are discarded.

`popn(0)` is a no-op. The Rust method accepts an `isize`; non-positive values are treated as a no-op. The Python method rejects negative values with `ValueError`.

## Persistence and identity

Operations do not mutate a `LeveledGSS`; they return a new value and retain `Arc`-backed sharing where possible. `ptr_eq` and `ptr_key` expose root-allocation identity for memoization. They do not express semantic equality and must not be persisted across processes.

## Materialization and counts

`to_stacks(max_stacks)` is for diagnostics, tests, and bounded interoperability. It returns `None` in Rust, or raises `OverflowError` in Python, rather than truncating when the structural path count exceeds the limit.

`path_count_at_most(limit)` counts structural graph paths, capped at `limit`. Structurally distinct paths can denote the same concrete value sequence, so this count can exceed the number of unique stacks returned after canonicalizing `to_stacks`.

`summary()` reports graph structure without materializing all paths.

## Segment backend

The internal linear-segment representation is selected once per process with the `STACKVEC` environment variable:

- `vec` (default): ordinary `Vec` storage;
- `arc`: copy-on-write `Arc<Vec<_>>` storage.

The choice affects performance and sharing, not public semantics. CI runs the full Rust suite against both backends.
