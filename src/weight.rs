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
}

impl Weight for () {
    #[inline]
    fn join(&self, _other: &Self) -> Self {}
}
