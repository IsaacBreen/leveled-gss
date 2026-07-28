# Benchmark methodology audit — 2026-07-28

This audit was prompted by an apparently surprising result: importing 1,024 already materialised weighted stacks took about 3.5 ms for `WeightedGss` but only tens of microseconds for an explicit hash map. The timing itself was real. Presenting it as representative “construction” was not.

## What was wrong

The original suite contained useful workloads, but its framing and several comparisons were weak:

1. A flat `Vec<(Vec<Symbol>, Weight)>` import was placed beside persistent operations as if both measured the same kind of construction. That input is already the explicit map's native representation; the map takes ownership and hashes it, while the GSS must discover and allocate shared structure.
2. Input cloning occurred inside constructor timing loops. Both implementations paid it, but the result mixed harness preparation with representation construction.
3. The explicit baselines did not reserve known capacity and rebuilt merges from both operands instead of cloning the larger and adding the smaller.
4. There was no benchmark that created the same exponentially branching language by applying the same public operation trace to both implementations.
5. Materialisation and borrowed visitation were mixed even though they produce different forms of output. The explicit visitor also did not enforce the same bound.
6. The old bounded GSS visitor used a second recursive materialisation algorithm that repeatedly created complete intermediate stack languages. The benchmark exposed a real implementation problem.
7. Throughput divided by the number of represented stacks made compact operations appear to process billions of stacks per second. That denominator describes denotation size, not work performed.
8. Adversarial broad-collapse cases were not clearly separated from representative structural workloads.

The old benchmarks were therefore not fabricated or universally unfair. The main error was asking different questions under one heading and then promoting the least representative conversion case in the README and article.

## Redesign

The audited suite now separates four questions:

- **Structural evolution:** both representations start from the same value and execute the same public push/merge trace.
- **Flat import:** both receive complete owned vectors; this deliberately measures conversion from an explicit enumeration.
- **Compact operations:** push, pop, merge, selection, and persistent forks operate on preconstructed equivalent states.
- **Concrete output:** materialisation and bounded visitation are measured separately from compact graph operations.

Constructor setup uses Criterion batches, so cloning inputs and destroying outputs occur outside the timed routine. Explicit maps and sets reserve capacity and use clone-the-larger merges. Integration tests prove that every structural builder produces the same extensional stack-to-weight map.

The structural binary trace is:

```text
value = merge(push(value, 2i), push(value, 2i + 1))
```

After 16 rounds it represents 65,536 distinct depth-16 stacks. The two-weight variant establishes two weight classes in the first round and preserves them through the remaining identical pushes and merges.

## Implementation defect found

`for_each_stack_top_first` was nominally a borrowed visitor, but for broad graphs it recursively constructed and memoised whole intermediate concrete languages before invoking callbacks. The audited implementation now reuses the direct bounded materialiser, reverses each completed stack in place, and then invokes the callback. It preserves the atomic-limit guarantee: if the complete result exceeds the bound, no callback is made.

On the 1,024-stack binary case, the old visitor took roughly 1.45 ms in a targeted run. The clean audited run measured about 93.8 µs. The explicit map still visits its already concrete stacks much faster, at about 5.67 µs; that is an honest consequence of the different representations.

## Clean audited run

- code commit: `9c8057a6e51d85922febad70d66b9f923b1e8e02`;
- benchmark machine: Apple M1 Pro;
- operating system: macOS / Darwin 25.5.0, arm64;
- toolchain: `rustc 1.91.0-nightly (54c581243 2025-08-25)`;
- command: `./scripts/run-benchmarks.sh -- --noplot` after removing `target/criterion`;
- recorded run: `target/benchmark-runs/20260728T085030Z`;
- source tree: clean;
- result: all Criterion targets completed successfully.

Criterion point estimates:

| Workload | `WeightedGss` | Explicit map | Interpretation |
|---|---:|---:|---|
| Structurally grow 16 binary levels, homogeneous weight | 8.33 µs | 23.7 ms | GSS work follows compact structure; explicit population doubles each round |
| Structurally grow 16 binary levels, two stable weights | 12.5 µs | 26.6 ms | Sharing remains effective across weight classes |
| Pop a structurally built 4,096-stack language | 104 ns | 235 µs | The compact representation removes one shared level |
| Persistent fork of a 512-stack value | 163 ns | 101 µs | The GSS reuses the immutable source |
| Merge values with a 20,000-symbol common top segment | 8.22 µs | 13.2 µs | Compact segment reuse avoids copying the full common run |
| Import an enumerated list of 1,024 weighted stacks | 3.57 ms | 15.1 µs | Flat import starts in the explicit representation's ideal form |
| Pop 1,024 independently weighted top branches into one stack | 117 µs | 42.2 µs | The explicit map remains faster on this adversarial broad collapse |
| Materialise 1,024 owned concrete stacks | 86.9 µs | 36.5 µs | Complete output pays for every concrete stack |
| Visit 1,024 concrete stacks with equivalent checksum work | 93.8 µs | 5.67 µs | Explicit storage is naturally better for repeated full enumeration |

The level-16 structural-pop GSS sample was noisy in the full sweep, so the table uses the stable level-12 result. The level-16 explicit result was about 4.84 ms while the GSS point estimate was about 170 ns.

## Scaling result

The structural construction series makes the distinction visible:

| Binary levels | Concrete stacks | GSS, two stable weights | Explicit map, two stable weights |
|---:|---:|---:|---:|
| 4 | 16 | 3.42 µs | 3.21 µs |
| 8 | 256 | 6.87 µs | 61.7 µs |
| 12 | 4,096 | 9.51 µs | 1.11 ms |
| 16 | 65,536 | 12.5 µs | 26.6 ms |

At tiny sizes the explicit map is competitive. As the concrete language doubles, its cost follows the number and depth of complete vectors. The GSS trace grows with the compact shared structure instead.

## Conclusions

- The flat-import benchmark is valid as an import/conversion measurement and remains in the suite under an explicit name.
- It is not representative evidence for a persistent GSS and is no longer the headline construction result.
- Structural traces, persistent operations, concrete output, and adversarial stress are now separate benchmark categories.
- The explicit baselines are competent enough that wins are meaningful.
- `WeightedGss` is not uniformly faster: flat import, broad collapse, and repeated complete enumeration still favour explicit storage.
- The benchmark audit found and fixed a real bounded-visitor performance defect.
