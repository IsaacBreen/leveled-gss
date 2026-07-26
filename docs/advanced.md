# Advanced facilities

## `Paths`

`WeightedGss::paths()` provides operations whose meaning is explicitly tied to stored weighted paths:

- bounded zero-copy callback traversal with `for_each_path_top_first`;
- capped structural path counts with `path_count_at_most`;
- immutable iteration over stored weight nodes without expanding paths;
- allocation-free access to the sole structural path with `with_single_path_top_first`;
- path-local map and filter-map operations, including fallible variants;
- partitioning paths by equal stored weights.

Weight-only transformations preserve the immutable unweighted stack DAG rather than rebuilding it. These operations are useful for dataflow annotations, parser exclusion state, diagnostics, and language bindings. They should not be confused with canonical iteration over distinct concrete stacks.

## `VirtualStack`

`VirtualStack` exposes a mutable linear top prefix. It is designed for deterministic parser and stack-machine loops that can operate directly until they reach ambiguity.

The object owns its persistent backing data and can be cheaply converted back into a `WeightedGss`. `apply_ops` branches several operations while sharing the unchanged lower region.

## `StackLanguageInterner`

The interner computes an exact key for the set of concrete stacks after erasing weights. It incrementally interns canonical top-first tries and memoises unions, allowing highly shared stack languages to remain compact.

Use one interner for one fixpoint computation. The returned `StackLanguageId` is meaningful only with that interner.
