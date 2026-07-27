//! Persistent weighted graph-structured stacks.
//!
//! A [`WeightedGss`] represents a finite collection of stack alternatives. Each
//! stack carries a [`Weight`], and weights are joined when operations make two
//! alternatives denote the same concrete stack.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "engine")]
pub mod engine;

mod gss;
mod nodes;
mod paths;
mod segment;
mod weight;
mod weight_regions;

#[cfg(feature = "python")]
mod python;

pub use gss::{Gss, PathLimitExceeded, WeightedGss};
pub use weight::Weight;
