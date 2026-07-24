//! A persistent, weighted graph-structured stack with leveled sharing.
//!
//! `weighted-gss` represents a set of stacks as one persistent graph. It denotes a finite map from complete stacks to weights, and [`Weight`]
//! defines how weights join when stack operations make two keys coincide. The implementation is extracted from GLRMask's
//! production parser data structure.
//!
//! # Basic use
//!
//! ```
//! use weighted_gss::WeightedGss;
//!
//! let left = WeightedGss::from_single_stack(vec![0_u32, 10, 20], ());
//! let right = WeightedGss::from_single_stack(vec![0_u32, 10, 30], ());
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
//! [`WeightedGss::to_stacks`] is intended for diagnostics and tests. Production
//! algorithms should usually operate on the shared representation directly.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::large_enum_variant, clippy::type_complexity)]

mod stack_vecs;
mod weighted_gss;

#[cfg(feature = "python")]
mod python;

pub use crate::weighted_gss::{VirtualStack, Weight, WeightedGss, WeightedGssSummary};
