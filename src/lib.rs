//! Persistent weighted graph-structured stacks.
//!
//! A [`WeightedGss`] represents a finite collection of stack alternatives. Each
//! stack carries a [`Weight`], and weights are joined when operations make two
//! alternatives denote the same concrete stack.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod gss;
mod linear_prefix;
mod materialize;
mod nodes;
mod segment;
mod stack_visit;
mod weight;
mod weight_regions;

#[cfg(feature = "python")]
mod python;

pub use gss::{Gss, WeightedGss};
pub use linear_prefix::{LinearPrefix, linear_prefix};
pub use stack_visit::{StackLimitExceeded, for_each_stack_top_first};
pub use weight::Weight;
