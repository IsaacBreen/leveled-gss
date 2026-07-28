# Benchmarks

The benchmark suite is kept in this repository because it measures the standalone crate's public operations and protects its implementation against regressions. It does not expose graph internals or add benchmark-only hooks to the library API.

## Baselines

The suite uses three private baseline representations from `benches/support/mod.rs`.

### Explicit stack-to-weight map

```text
FxHashMap<Vec<Symbol>, Weight>
```

This directly implements the documented semantics. Every operation copies or transforms concrete stacks and joins weights on key collision. It is both the conceptual reference implementation and the primary performance baseline.

It is not presented as an alternative production library. Its purpose is to show where graph sharing pays for itself and where a simple explicit representation remains competitive.

### Weight-partitioned stack sets

```text
FxHashMap<Weight, FxHashSet<Vec<Symbol>>>
```

This is the `weight -> GSS` architectural ablation. The same concrete stack may occur in several weight buckets; materialisation joins those weights.

The benchmark-only implementation requires hashable weights, unlike the public `WeightedGss` API. It measures the value and cost of sharing structure across distinct weights.

### Explicit unweighted stack set

```text
FxHashSet<Vec<Symbol>>
```

This is compared with `Gss<S> = WeightedGss<S, ()>`. It isolates the persistent graph representation from nontrivial weight handling.

## Workloads

The suite is divided into four Criterion targets.

### `construction`

- one linear stack at several depths;
- 128 stacks with a shared floor;
- a binary language containing 1,024 concrete stacks;
- one, two, eight, and 32 distinct weights over the same stack shape.

### `operations`

- push and pop on linear stacks;
- half-overlapping merges;
- join-heavy pop, where many alternatives collapse to one stack;
- `retain_top` over increasingly broad frontiers;
- persistent forks from one immutable source.

### `materialization`

- `to_stacks`;
- `for_each_stack_top_first`;
- explicit-map snapshots;
- weight-partitioned canonicalisation;
- early rejection when a concrete-stack bound is too small.

Materialisation is kept separate because its cost is inherently proportional to concrete output.

### `unweighted`

- construction, push, pop, merge, and materialisation;
- `Gss<S>` against an explicit stack set.

## Running benchmarks

Run everything:

```bash
cargo bench
```

Run one target or one group:

```bash
cargo bench --bench operations
cargo bench --bench operations -- operations/join_heavy_pop
```

The repository helper records the commit, toolchain, operating system, CPU description, dirty state, and benchmark output under `target/benchmark-runs/`:

```bash
./scripts/run-benchmarks.sh
./scripts/run-benchmarks.sh --bench operations -- operations/join_heavy_pop
```

Criterion baselines can compare a candidate against an earlier run on the same machine:

```bash
cargo bench -- --save-baseline main
# switch commits or branches without changing the machine configuration
cargo bench -- --baseline main
```

## Interpreting results

A useful result answers a specific question about a shape and operation. There is no single “GSS speed” number.

Record at least:

- commit and dirty state;
- Rust version;
- CPU and operating system;
- power mode and competing load;
- benchmark target, shape, and size;
- whether the result includes construction or materialisation.

Do not use absolute timing gates on shared GitHub-hosted runners. CI compiles and smoke-executes the benchmark targets; regression decisions should use adjacent Criterion baselines on the same isolated machine.

The historical full GLRMask/CFA timing bracket remains an integration benchmark outside this standalone suite. It answers whether the API can support one demanding parser application, not how the general data structure scales in isolation.
