# Semantics and invariants

## Weighted alternatives

A `WeightedGss<S, W>` represents a finite mapping from concrete stacks to weights. Stacks are ordered bottom-to-top.

Internally, several structural paths may spell the same concrete stack. The observable weight of that stack is the join of all corresponding path weights:

```text
meaning(gss, stack) = join(path weights for paths spelling stack)
```

Graph sharing, path duplication, and eager coalescing are implementation details.

## Weight laws

Weights implement ordinary equality. Equal weights may be factored over one shared stack language.

`Weight::join` must be:

- associative: `(a ⋁ b) ⋁ c = a ⋁ (b ⋁ c)`;
- commutative: `a ⋁ b = b ⋁ a`;
- idempotent: `a ⋁ a = a`.

Set union, bitwise OR, minimum, and maximum are typical joins. Integer addition is generally not valid because it is not idempotent.

## Empty GSS and empty stack

These are different:

- `WeightedGss::new()` contains no alternatives;
- `WeightedGss::from_stack([], weight)` contains one alternative: the empty stack.

`is_empty()` tests the first condition. `has_empty_stack()` tests the second.

## Stack operations

`push(x)` appends `x` to every represented stack.

`pop()` removes one top value from every non-empty stack. Empty alternatives underflow and disappear.

`popn(n)` removes exactly `n` values. Stacks shorter than `n` disappear, stacks of length `n` become empty, and `popn(0)` is an identity operation.

When operations make two stacks coincide, their weights are joined.

## Top selection

`top()` returns a value only when there is exactly one distinct non-empty top value and no empty-stack alternative.

`tops()` iterates over each distinct non-empty top value once. Its order is unspecified.

`retain_top(x)` keeps stacks topped by `x` without popping. `pop_top(x)` keeps the same alternatives and removes `x`.

## Observations

`joined_weight()` joins the weights of every represented alternative and returns `None` only for an empty GSS.

`to_stacks(max_paths)` returns canonical bottom-to-top stacks with coincident weights joined. The bound limits internal structural paths traversed, not merely the number of output entries. Exceeding it returns `PathLimitExceeded`; materialisation never silently truncates.

## Persistence

Operations return new values and retain immutable sharing where possible. Existing values remain usable.

`WeightedGss` deliberately does not implement equality or hashing. Representation equality, raw-path equality, and equality of the extensional stack-to-weight mapping are distinct concepts.

## Deliberately excluded API

The crate does not expose raw graph paths, representation IDs, virtual-stack views, batched parser stack effects, structural profiling, or canonical stack-language IDs.

Those mechanisms were useful while stress-testing the implementation inside GLRMask, but they are not part of the weighted-stack abstraction and are not required to use the crate.
