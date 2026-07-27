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

## Factored weight regions

The representation may factor one weight over a shared language containing many
stacks. Conversely, equal weight values may occur in several distinct stored
regions. Therefore `weights()` does not yield one item per stack: its order and
item count are unspecified and are not semantic properties.

`map_weights(f)` and `filter_map_weights(f)` apply `f` once to each distinct
reachable stored weight region while preserving graph sharing. They do not first
materialise stacks or compute each stack's joined weight. `None` from
`filter_map_weights` removes the complete stack sublanguage covered by that
region.

No additional algebraic law is required to use these operations. When the result
must be independent of equivalent internal refactorings, however, the transform
must preserve joins. For `map_weights`:

```text
f(a ⋁ b) = f(a) ⋁ f(b)
```

For `filter_map_weights`, treat `None` as no contribution and define the join of
two `Some` values using `V::join`; the lifted transform must preserve joins under
that operation. Callbacks that do not satisfy this condition intentionally observe
the current weight factorisation.

There is no mutable weight iterator. A `WeightedGss` is persistent, and one stored
weight may be shared by many stacks and graph parents.

## Empty GSS and empty stack

These are different:

- `WeightedGss::new()` contains no alternatives;
- `WeightedGss::from_stack([], weight)` contains one alternative: the empty stack.

`is_empty()` tests the first condition. `has_empty_stack()` tests the second. `retain_empty()` selects only that empty-stack alternative.

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

`to_stacks(max_stacks)` returns canonical bottom-to-top stacks with coincident weights joined. The bound is the number of distinct concrete stacks in the result. Exceeding it returns the opaque `StackLimitExceeded`; materialisation never silently truncates and does not expose encoded-path counts.

`for_each_stack_top_first(gss, max_stacks, visit)` uses the same semantic bound while presenting each stack as a borrowed top-first slice.

## Persistence

Operations return new values and retain immutable sharing where possible. Existing values remain usable.

`WeightedGss` deliberately does not implement equality or hashing. Representation equality, raw-path equality, and equality of the extensional stack-to-weight mapping are distinct concepts.

## Linear-prefix fast path

`linear_prefix(gss)` exposes a mutable `LinearPrefix` only when the representation has one homogeneous weight and a directly accessible linear top prefix. Its hidden floor may remain branched. Converting the view back into a `WeightedGss` preserves that unchanged floor.

The public API does not expose graph paths, representation IDs, structural profiling, parser stack effects, or canonical stack-language machinery.
