# Python API

The Python package exposes the ordinary semantic API of the Rust implementation. Wheels use CPython's stable ABI and support Python 3.8 and later. Python wrapper instances are thread-affine; create and use a `WeightedGSS` on the same Python thread.

```python
from weighted_gss import WeightedGSS, __version__
```

## Values and weights

Stacks are supplied and returned bottom-to-top. Stack values must be immutable and hashable for as long as the GSS exists.

A weight is either `None`, for unweighted use, or an object implementing:

```python
def join(self, other): ...
```

`join` must be associative, commutative, and idempotent. Weights do not need to be hashable. Exceptions raised by `join` or stack-value equality are propagated normally to Python.

## Construction

```python
empty = WeightedGSS()
one = WeightedGSS.from_stack([0, 1, 2], weight)
weighted = WeightedGSS.from_stacks([
    ([0, 1, 2], weight_a),
    ([0, 1, 3], weight_b),
])
unweighted = WeightedGSS.from_unweighted([[0, 1, 2], [0, 1, 3]])
updated = weighted.with_stack([0, 4], another_weight)
```

All operations are persistent: the original value remains usable.

## Stack operations

- `push(value)` pushes onto every represented stack.
- `pop()` removes one value and discards empty alternatives.
- `popn(count)` removes `count` values and discards underflowing alternatives.
- `merge(other)` and `merge_all(values)` combine alternatives.

## Top frontier

- `tops()` returns the distinct non-empty top values.
- `top()` returns the unique top value, and raises `ValueError` when the frontier is empty or ambiguous, or when an empty-stack alternative is also present. This allows `None` itself to remain a valid stack symbol.
- `has_empty_stack()` reports an empty-stack alternative.
- `retain_top(value)` selects matching alternatives without popping.
- `retain_empty()` selects the empty stack.
- `pop_top(value)` selects and pops one top branch.
- `pop_branches()` returns `(top, remainder)` pairs for every non-empty top branch.

## Weights and inspection

- `joined_weight()` joins every represented path weight and raises `ValueError` when the GSS is empty.
- `empty_weight()` returns the joined weight of the empty stack and raises `ValueError` when no empty stack exists.

Both methods may legitimately return `None` for an unweighted GSS; absence is therefore reported by an exception rather than overloaded onto `None`.
- `is_empty()` and Boolean conversion test whether alternatives exist.
- `max_depth()` returns the maximum stack depth.
- `to_stacks(max_paths=4096)` materialises canonical `(stack, weight)` pairs. It raises `OverflowError` rather than silently traversing more than the requested number of structural paths.

The Python binding intentionally does not expose raw structural paths, representation IDs, the stack-language interner, or `VirtualStack`. Those are Rust facilities for implementing high-performance stack machines.
