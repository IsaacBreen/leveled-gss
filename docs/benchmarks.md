# Benchmarks

The benchmark suite measures the standalone crate's public semantics. It does not inspect graph nodes or add benchmark-only library hooks.

## Comparison rules

A benchmark comparison must answer one specific question. The suite follows these rules:

1. **Use the same public operation trace.** Structural-growth cases start from the same extensional value and apply the same pushes, merges, pops, and weight assignments to each representation.
2. **Keep conversion separate from structural evolution.** `from_stacks` benchmarks intentionally begin with complete concrete vectors and are labelled `from_explicit_entries`; they measure import cost, not general scalability.
3. **Exclude harness preparation.** Constructor inputs are cloned in Criterion setup batches, outside the timed routine. Criterion also drops constructed results outside the timed interval.
4. **Compare equivalent output.** Owned-output cases produce complete concrete `(stack, weight)` values. Borrowed-visit cases perform the same orientation-sensitive checksum and enforce the same stack-count bound.
5. **Verify benchmark helpers.** Integration tests compare each weighted and unweighted structural builder against the compact implementation after every complete trace.
6. **Label stress cases as stress cases.** Broad fan-out collapse and other adversarial shapes remain valuable regression tests, but they are not presented as typical parser workloads.
7. **Do not derive throughput from represented alternatives for compressed operations.** A constant-time operation over a compact language may represent exponentially many stacks; dividing by that denotation produces impressive but misleading rates.

## Baselines

### Explicit stack-to-weight map

```text
FxHashMap<Vec<Symbol>, Weight>
```

This directly implements the documented finite-map semantics. It reserves capacity when input size is known. Merge clones the larger operand and inserts the smaller, joining weights on key collision. Push, pop, and selection transform complete concrete stacks.

It is the primary semantic and performance baseline, not a deliberately slow straw man.

### Weight-partitioned stack sets

```text
FxHashMap<Weight, FxHashSet<Vec<Symbol>>>
```

This benchmark-only ablation asks what happens when sharing is allowed inside each weight class but not across weights. The same concrete stack may occur in several buckets, so materialisation must join coincident weights. It requires hashable weights, unlike the public API.

### Explicit unweighted stack set

```text
FxHashSet<Vec<Symbol>>
```

This is compared with `Gss<S> = WeightedGss<S, ()>`. It isolates persistent graph sharing from nontrivial weight handling.

## Workloads

### `construction`

`construction/from_owned_single_stack` measures taking ownership of one complete stack at several depths.

`construction/from_explicit_entries` measures conversion from already enumerated inputs:

- 128 stacks sharing a bottom segment;
- all 1,024 paths of a depth-10 binary language;
- one, two, eight, and 32 weight classes;
- two stacks sharing up to 20,000 top symbols.

These are valid import benchmarks but are deliberately not used as the primary scaling evidence.

`construction/structural_binary_growth` starts from an empty stack language and performs the same repeated public trace on both implementations:

```text
value = merge(push(value, left_symbol), push(value, right_symbol))
```

After `d` rounds the value denotes `2^d` stacks. The suite measures both one homogeneous weight and two stable weight classes at depths 4, 8, 12, and 16.

### `operations`

- push and pop on one linear stack at several depths;
- disjoint, half-overlapping, and completely overlapping merges;
- independently constructed values with common top segments up to 20,000 symbols;
- `structural_binary_pop`, operating on values built through the structural trace;
- `retain_top` over increasingly broad frontiers;
- persistent forks from one immutable source;
- `stress/wide_fanout_collapse_pop`, where many independently weighted top branches collapse simultaneously.

### `materialization`

- `owned_output`: complete concrete outputs from `to_stacks`, explicit-map snapshots, and weight-partitioned materialisation;
- `borrowed_visit`: bounded visits with equivalent top-first checksumming work;
- `limit_rejection`: failure before callbacks when the distinct-stack limit is too small.

Materialisation is inherently proportional to concrete output. It is kept separate from compact graph operations.

### `unweighted`

The unweighted target mirrors flat import, structural binary growth, push, pop, materialisation, structural pop, and merge against an explicit stack set.

## Running benchmarks

Run everything and record environment metadata:

```bash
./scripts/run-benchmarks.sh -- --noplot
```

Run one target or group:

```bash
cargo bench --bench construction -- structural_binary_growth
cargo bench --bench operations -- structural_binary_pop
cargo bench --bench operations -- stress/wide_fanout_collapse_pop
```

Criterion baselines can compare adjacent commits on the same machine:

```bash
cargo bench -- --save-baseline main
# switch commits without changing machine configuration
cargo bench -- --baseline main
```

## Interpreting results

There is no single “GSS speed” number. Record at least the commit, dirty state, toolchain, CPU, operating system, competing load, exact workload, input shape, and whether construction or materialisation is included.

Do not use absolute timing gates on shared GitHub-hosted runners. CI compiles and smoke-executes the benchmarks; performance decisions should use adjacent runs on the same isolated machine.

The historical GLRMask/CFA timing bracket remains application-level evidence. It answers whether the API supports one demanding parser workload, not how the standalone structure scales in isolation.

See [Benchmark methodology audit — 2026-07-28](validation/benchmark-audit-2026-07-28.md) for the redesign rationale and a clean recorded run.
