# Python API

The Python package exposes the Rust implementation through PyO3. Wheels use CPython's stable ABI and support Python 3.8 and later.

```python
from leveled_gss import LeveledGSS, LeveledGSSSummary, __version__
```

## Object requirements

Stack values must be immutable and hashable for the lifetime of the GSS. Equality and hash behavior must remain stable.

Weighted accumulators have the same requirement and must implement:

```python
def merge(self, other): ...
```

`merge` must return an accumulator of the same semantic family and act as an associative, commutative, idempotent join. Use `LeveledGSS.from_unweighted` to use `None` automatically.

`LeveledGSS` instances are thread-affine in the Python binding. Create and use an instance on the same Python thread. Rust `LeveledGSS` values are not subject to this Python wrapper restriction.

## Construction

- `LeveledGSS()` and `LeveledGSS.empty()` create a GSS with no paths.
- `from_single_stack(stack, accumulator=None)` creates one path.
- `from_unweighted(stacks)` accepts any outer iterable of bottom-to-top sequences.
- `from_stacks(items)` accepts any outer iterable of `(stack, accumulator)` pairs.

Duplicate stacks are joined through the accumulator's `merge` method.

## Core operations

- `push(value)` pushes onto all paths.
- `pop()` removes one value and discards underflowing paths.
- `popn(count)` removes `count` values and rejects negative counts.
- `isolate(value)` keeps matching top values; `isolate(None)` keeps empty-stack paths.
- `merge(other)` and `merge_many(gsses)` form unions.
- `peek()` returns distinct non-empty top values.
- `reduce_acc()` joins all distinct stored accumulators.

All operations return new GSS values.

## Bounded inspection

`to_stacks(max_stacks=4096)` returns a list of `(list, accumulator)` pairs. It raises `OverflowError` if the structural path count exceeds the bound.

`path_count_at_most(limit)` returns the structural graph-path count capped at `limit`; it is not necessarily a unique-stack count.

`summary()` returns `LeveledGSSSummary`, which exposes node, edge, frontier, accumulator, and depth statistics.

## Typing and introspection

The distribution includes `py.typed` and a `.pyi` stub. Runtime methods and classes also contain docstrings:

```python
help(LeveledGSS)
help(LeveledGSS.to_stacks)
```
