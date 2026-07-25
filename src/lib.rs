//! Persistent weighted graph-structured stacks.
//!
//! A [`WeightedGss`] represents weighted stack alternatives using a persistent
//! shared graph. Each structural path spells a stack and carries a [`Weight`].
//! Several structural paths may spell the same concrete stack; their extensional
//! weight is the join of the corresponding path weights.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod diagnostics;
mod effects;
mod gss;
mod language;
mod nodes;
mod paths;
mod segment;
mod virtual_stack;
mod weight;

#[cfg(feature = "python")]
mod python;

pub use diagnostics::{RepresentationId, StructuralStats};
pub use effects::StackEffect;
pub use gss::{Gss, PathLimitExceeded, TopBranch, TopBranches, Tops, WeightedGss};
pub use language::{StackLanguageId, StackLanguageInterner};
pub use paths::Paths;
pub use virtual_stack::VirtualStack;
pub use weight::Weight;
