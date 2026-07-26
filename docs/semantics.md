# Semantics and invariants

## Weighted alternatives

A `WeightedGss<S, W>` is a persistent graph representation of weighted stack alternatives. A structural path spells a bottom-to-top stack and carries a weight.

The extensional weight of a concrete stack is the join of all structural paths spelling that stack:

```text
meaning(gss, stack) = join(path weights for paths spelling stack)
```

Normal stack operations preserve this extensional meaning. The implementation may join colliding alternatives eagerly and may share or restructure graph nodes without changing the meaning.

## Weight laws

`Weight::join` must be:

- associative: `(a ⋁ b) ⋁ c = a ⋁ (b ⋁ c)`;
- commutative: `a ⋁ b = b ⋁ a`;
- idempotent: `a ⋁ a = a`.

Integer addition is therefore not generally a valid join. Set union, bitwise OR, minimum, and maximum are typical examples.

## Empty GSS and empty stack

These are distinct:

- `WeightedGss::new()` contains no alternatives.
- `WeightedGss::from_stack([], weight)` contains the empty stack.

`is_empty()` tests the first condition. `has_empty_stack()`, `retain_empty()`, and `empty_weight()` operate on the second.

## Push, pop, and stack operations

`push(x)` appends `x` to every concrete stack.

`pop()` removes one top value from every non-empty alternative. Empty alternatives underflow and disappear.

`popn(n)` removes exactly `n` values. Stacks shorter than `n` disappear; stacks of length `n` become empty; `popn(0)` is an identity operation.

A `StackOp` records a pop count followed by a pushed sequence. The final pushed item becomes the new top. `apply_ops` represents nondeterministic choice among operations.

When any of these operations cause concrete stacks to coincide, their weights join.

## Top operations

`top()` returns a value only when there is exactly one distinct non-empty top symbol and no empty-stack alternative.

`tops()` yields each distinct non-empty top symbol once. Ordering is unspecified.

`retain_top(x)` keeps stacks topped by `x` without popping it. `pop_top(x)` keeps the same branch and removes `x`. `pop_branches()` returns all distinct top branches already popped.

`retain_where_at_depth(0, predicate)` examines the top. Larger depths count downward from the top. Too-short stacks are discarded.

## Path-local operations

`gss.paths()` explicitly crosses from extensional stack operations into operations on the currently stored path weights.

For example, `paths().map_weights(f)` applies `f` to stored path-local weights. In general this is not equivalent to applying `f` after joining equal concrete stacks:

```text
f(a ⋁ b) may differ from f(a) ⋁ f(b)
```

The explicit `paths()` boundary prevents that distinction from being hidden behind an ordinary-looking map operation.

Raw path traversal exposes representation paths directly. Their count, ordering, weight placement, and degree of eager coalescing are implementation details. Callers must not infer extensional distinct-stack counts from them or rely on a particular layout.

## Materialisation

`to_stacks(max_paths)` returns canonical concrete stacks with joined weights. `max_paths` bounds structural paths traversed, not merely output entries. The operation returns `PathLimitExceeded` rather than silently truncating.

Both are diagnostic and interoperability operations, not intended for hot stack manipulation.

## Persistence and identity

Operations return new values and retain immutable sharing where possible.

`WeightedGss` deliberately does not implement ordinary equality or hashing because representation equality, raw-path equality, and extensional equality are different concepts.

## Virtual stacks

`try_virtual_stack()` is a conservative optimisation probe. It succeeds when the current representation has a common linear top prefix.

The prefix may end at a hidden branched floor. `prefix_len()` counts only visible linear values. `pop_prefix(n)` returns the number of requested pops that crossed beyond that prefix. `is_complete()` is true only when the hidden floor is exactly the empty stack.

Failure to obtain a virtual stack says nothing about the number of extensional concrete stacks; it only says the current representation cannot expose the fast path cheaply.

## Stack-language interning

`StackLanguageInterner` assigns exact canonical IDs to the unweighted set of concrete stacks. It ignores weights and graph layout. It is suitable for visited sets in reduction closures and other fixpoint algorithms.

IDs are local to one interner. The interner retains enough internal graph identity to prevent allocator pointer reuse from corrupting later keys.
