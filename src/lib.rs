//! A persistent, weighted graph-structured stack with leveled sharing.
//!
//! `leveled-gss` represents a set of stacks as one persistent graph. Paths can
//! carry an accumulator, and [`Merge`] defines how accumulators combine when
//! equivalent paths meet. The implementation is extracted from GLRMask's
//! production parser data structure.
//!
//! # Basic use
//!
//! ```
//! use leveled_gss::LeveledGSS;
//!
//! let left = LeveledGSS::from_single_stack(vec![0_u32, 10, 20], ());
//! let right = LeveledGSS::from_single_stack(vec![0_u32, 10, 30], ());
//! let merged = left.merge(&right).push(40);
//!
//! let mut stacks = merged.to_stacks(8).unwrap();
//! stacks.sort_by(|a, b| a.0.cmp(&b.0));
//! assert_eq!(
//!     stacks,
//!     vec![
//!         (vec![0, 10, 20, 40], ()),
//!         (vec![0, 10, 30, 40], ()),
//!     ],
//! );
//! ```
//!
//! [`LeveledGSS::to_stacks`] is intended for diagnostics and tests. Production
//! algorithms should usually operate on the shared representation directly.

#![cfg_attr(not(feature = "python"), forbid(unsafe_code))]
#![cfg_attr(
    feature = "python",
    allow(unsafe_op_in_unsafe_fn, non_local_definitions)
)]
#![allow(dead_code)]
#![allow(clippy::large_enum_variant, clippy::type_complexity)]

mod leveled_gss;
mod stack_vecs;

#[cfg(feature = "python")]
mod python;

pub use crate::leveled_gss::{LeveledGSS, LeveledGSSSummary, Merge, VirtualStack};
