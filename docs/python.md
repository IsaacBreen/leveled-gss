# Python API

The Python package exposes the Rust implementation through PyO3. Wheels use CPython's stable ABI and support Python 3.8 and later.

```python
from weighted_gss import WeightedGSS, WeightedGSSSummary, __version__
```

## Object requirements

Stack values must be immutable and hashable for the lifetime of the GSS. Equality and hash behavior must remain stable.

Weights have the same requirement and must implement:

```python
def join(self, other): ...
```

`join` must return a weight of the same semantic family and act as an associative, commutative, idempotent join. Use `WeightedGSS.from_unweighted` to store `None` automatically.

`WeightedGSS` instances are thread-affine in the Python binding. Create and use an instance on the same Python thread. Rust `WeightedGss` values are not subject to this Python-wrapper restriction.

## Construction

- `WeightedGSS()` and `WeightedGSS.empty()` create a GSS with no entries.
- `from_single_stack(stack, weight=None)` creates one entry.
- `from_unweighted(stacks)` accepts an iterable of bottom-to-top stacks.
- `from_stacks(items)` accepts an iterable of `(stack, weight)` pairs.

Duplicate stacks are combined through the weight's `join` method.

## Core operations

- `push(value)` pushes onto all stacks.
- `pop()` removes one value and discards underflowing entries.
- `popn(count)` removes `count` values and rejects negative counts.
- `isolate(value)` keeps matching top values; `isolate(None)` keeps empty-stack entries.
- `merge(other)` and `merge_many(gsses)` form pointwise unions.
- `peek()` returns distinct non-empty top values.
- `join_weights()` joins all distinct stored weights.

All operations return new GSS values.

## Bounded inspection

`to_stacks(max_stacks=4096)` returns `(stack, weight)` pairs. It raises `OverflowError` if the structural path count exceeds the bound.

`path_count_at_most(limit)` returns the structural graph-path count capped at `limit`; it is not necessarily a distinct-stack count.

`summary()` returns `WeightedGSSSummary`, which exposes node, edge, frontier, weight, and depth statistics.

## Typing and introspection

The distribution includes `py.typed` and a `.pyi` stub. Runtime methods and classes also contain docstrings:

```python
help(WeightedGSS)
help(WeightedGSS.to_stacks)
```
