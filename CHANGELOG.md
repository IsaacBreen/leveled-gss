# Changelog

## Unreleased

- Extract the production leveled graph-structured stack from GLRMask.
- Expose the persistent GSS, virtual-stack fast path, accumulator merge trait, and structural summary.
- Add PyO3/maturin Python bindings for weighted and unweighted use.
- Add randomized equivalence testing against an explicit stack-set model.
- Test both internal segment backends and a 262,144-path compressed graph.
- Correct inherited parser-floor behavior so underflowing `popn` paths are discarded.
- Add Linux, macOS, and Windows CI for Rust and Python.
