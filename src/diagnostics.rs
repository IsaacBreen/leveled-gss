/// Opaque process-local identity of a persistent GSS representation.
///
/// Equal IDs mean the values share the same root representation. Different IDs
/// do not imply different extensional meanings. IDs must not be serialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RepresentationId(pub(crate) usize);

impl RepresentationId {
    /// Return the compact process-local integer identifier.
    ///
    /// The value is opaque: callers may use it for indexing or compatibility
    /// with integer-keyed caches, but must not interpret it as an address or
    /// persist it across processes.
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

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
