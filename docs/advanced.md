# Advanced facilities

## `Paths`

`WeightedGss::paths()` provides operations whose meaning is explicitly tied to stored weighted paths:

- bounded raw materialisation with `to_vec`;
- bounded zero-copy callback traversal with `for_each_top_first`;
- capped structural path counts;
- caller-buffer extraction of the only structural path, including `single_top_first_small` for allocation-free inline buffers;
- visiting stored weight nodes without expanding paths;
- path-local map and filter-map operations, including fallible variants;
- partitioning paths by equal stored weights.

Weight-only transformations preserve the immutable unweighted stack DAG, so changing annotations does not rebuild the stack structure. These operations are useful for dataflow annotations, parser exclusion state, diagnostics, and language bindings. They should not be confused with canonical iteration over distinct concrete stacks.

## `VirtualStack`

`VirtualStack` exposes a mutable linear top prefix. It is designed for deterministic parser and stack-machine loops that can operate directly until they reach ambiguity.

The object owns its persistent backing data and can be cheaply converted back into a `WeightedGss`. `apply_effects` branches several effects while sharing the unchanged lower region.

## `StackLanguageInterner`

The interner computes an exact key for the set of concrete stacks after erasing weights. It incrementally interns canonical top-first tries and memoises unions, allowing highly shared stack languages to remain compact.

Use one interner for one fixpoint computation. The returned `StackLanguageId` is meaningful only with that interner.

## Diagnostics

`representation_id()` supports process-local memoisation by root identity. IDs are assigned lazily, remain stable across clones, and are not reused during the process lifetime.

`structural_stats()` reports representation-level nodes, edges, paths, and maximum depth. These values are diagnostics, not semantic equality criteria, and may change as the implementation evolves.
