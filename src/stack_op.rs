/// A stack operation that pops values and then pushes a sequence.
///
/// Values in `push` are pushed in iteration order, so the final value becomes
/// the new top of the stack.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StackOp<P> {
    pub(crate) pop: usize,
    pub(crate) push: P,
}

impl<P> StackOp<P> {
    /// Construct an operation that pops `pop` values and then pushes `push`.
    #[must_use]
    pub const fn new(pop: usize, push: P) -> Self {
        Self { pop, push }
    }

    /// Return the number of values popped by this operation.
    #[must_use]
    pub const fn pop_count(&self) -> usize {
        self.pop
    }

    /// Return the pushed sequence container.
    #[must_use]
    pub const fn pushed(&self) -> &P {
        &self.push
    }
}
