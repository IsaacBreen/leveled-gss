/// A value associated with a structural stack path.
///
/// [`Weight::join`] must be associative, commutative, and idempotent. These
/// laws make the extensional meaning independent of representation choices:
///
/// - `(a ⋁ b) ⋁ c = a ⋁ (b ⋁ c)`
/// - `a ⋁ b = b ⋁ a`
/// - `a ⋁ a = a`
pub trait Weight: Clone {
    /// Join two weights.
    fn join(&self, other: &Self) -> Self;

    /// Return whether two values are known to denote the same weight.
    ///
    /// This is an optional sharing hint. The default is conservative: returning
    /// `false` never changes semantics, but may miss homogeneous-DAG fast paths.
    /// Returning `true` must imply that joining the values is equivalent to
    /// retaining either one.
    fn equivalent(&self, _other: &Self) -> bool {
        false
    }
}

impl Weight for () {
    #[inline]
    fn join(&self, _other: &Self) -> Self {}

    #[inline]
    fn equivalent(&self, _other: &Self) -> bool {
        true
    }
}
