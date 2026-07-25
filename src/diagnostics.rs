/// Opaque process-local identity of a persistent GSS representation.
///
/// Equal IDs mean the values share the same root representation. Different IDs
/// do not imply different extensional meanings. IDs must not be serialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RepresentationId(pub(crate) usize);

/// Representation-level statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StructuralStats {
    /// Number of unique graph nodes reachable from the root.
    pub nodes: usize,
    /// Number of graph edges between unique nodes.
    pub edges: usize,
    /// Number of structural weighted paths, saturating at [`usize::MAX`].
    pub paths: usize,
    /// Maximum concrete stack depth.
    pub max_depth: usize,
}
