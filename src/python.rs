use crate::{Weight, WeightedGss as CoreWeightedGss, WeightedGssSummary as CoreSummary};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyOverflowError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList, PyModule, PySet, PyTuple, PyType};
use std::hash::{Hash, Hasher};

struct PyKey(Py<PyAny>);

impl Clone for PyKey {
    fn clone(&self) -> Self {
        Python::attach(|py| Self(self.0.clone_ref(py)))
    }
}

fn objects_equal(left: &Py<PyAny>, right: &Py<PyAny>) -> bool {
    Python::attach(|py| {
        left.bind(py)
            .rich_compare(right.bind(py), CompareOp::Eq)
            .and_then(|result| result.is_truthy())
            .unwrap_or(false)
    })
}

impl PartialEq for PyKey {
    fn eq(&self, other: &Self) -> bool {
        objects_equal(&self.0, &other.0)
    }
}

impl Eq for PyKey {}

impl Hash for PyKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Python::attach(|py| self.0.bind(py).hash().unwrap_or(0).hash(state));
    }
}

struct PyWeight(Py<PyAny>);

impl Clone for PyWeight {
    fn clone(&self) -> Self {
        Python::attach(|py| Self(self.0.clone_ref(py)))
    }
}

impl PartialEq for PyWeight {
    fn eq(&self, other: &Self) -> bool {
        objects_equal(&self.0, &other.0)
    }
}

impl Eq for PyWeight {}

impl Hash for PyWeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Python::attach(|py| self.0.bind(py).hash().unwrap_or(0).hash(state));
    }
}

impl Weight for PyWeight {
    fn join(&self, other: &Self) -> Self {
        Python::attach(|py| {
            if self.0.bind(py).is_none() && other.0.bind(py).is_none() {
                return Self(py.None());
            }
            let merged = self
                .0
                .call_method1(py, "join", (other.0.clone_ref(py),))
                .unwrap_or_else(|error| {
                    error.restore(py);
                    panic!("Python weight join() raised an exception")
                });
            Self(merged)
        })
    }

    fn subsumes(&self, other: &Self) -> bool {
        self == other
    }
}

fn validate_hashable(py: Python<'_>, object: &Py<PyAny>, kind: &str) -> PyResult<()> {
    object
        .bind(py)
        .hash()
        .map(|_| ())
        .map_err(|_| PyTypeError::new_err(format!("{kind} must be hashable")))
}

/// Structural statistics for a shared graph-structured stack.
#[pyclass(
    name = "WeightedGSSSummary",
    module = "weighted_gss._native",
    skip_from_py_object
)]
#[derive(Clone)]
struct PyWeightedGSSSummary {
    /// Number of distinct values at the top frontier.
    #[pyo3(get)]
    top_values_count: usize,
    /// Number of weight-carrying branch nodes.
    #[pyo3(get)]
    upperbranch_nodes: usize,
    /// Number of interface nodes.
    #[pyo3(get)]
    interface_nodes: usize,
    /// Total number of lower-layer nodes.
    #[pyo3(get)]
    lower_nodes: usize,
    /// Number of branching lower-layer nodes.
    #[pyo3(get)]
    lower_general_nodes: usize,
    /// Number of compact linear segment nodes.
    #[pyo3(get)]
    lower_segment_nodes: usize,
    /// Total number of unique graph nodes.
    #[pyo3(get)]
    total_unique_nodes: usize,
    /// Total number of graph edges.
    #[pyo3(get)]
    total_edges: usize,
    /// Number of stored weight instances.
    #[pyo3(get)]
    weight_instances: usize,
    /// Maximum represented stack depth.
    #[pyo3(get)]
    max_depth: u32,
}

impl From<CoreSummary> for PyWeightedGSSSummary {
    fn from(summary: CoreSummary) -> Self {
        Self {
            top_values_count: summary.top_values_count,
            upperbranch_nodes: summary.upperbranch_nodes,
            interface_nodes: summary.interface_nodes,
            lower_nodes: summary.lower_nodes,
            lower_general_nodes: summary.lower_general_nodes,
            lower_segment_nodes: summary.lower_segment_nodes,
            total_unique_nodes: summary.total_unique_nodes,
            total_edges: summary.total_edges,
            weight_instances: summary.weight_instances,
            max_depth: summary.max_depth,
        }
    }
}

#[pymethods]
impl PyWeightedGSSSummary {
    fn __repr__(&self) -> String {
        format!(
            "WeightedGSSSummary(top_values={}, nodes={}, edges={}, max_depth={})",
            self.top_values_count, self.total_unique_nodes, self.total_edges, self.max_depth
        )
    }
}

/// A persistent set of stack paths with shared structure and optional weights.
///
/// Stack values and weights must be immutable and hashable. Weights must provide a ``join(other)`` method implementing an
/// associative, commutative, idempotent join.
#[pyclass(
    name = "WeightedGSS",
    module = "weighted_gss._native",
    unsendable,
    skip_from_py_object
)]
struct PyWeightedGSS {
    inner: CoreWeightedGss<PyKey, PyWeight>,
}

impl Clone for PyWeightedGSS {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl PyWeightedGSS {
    fn stacks_to_python(&self, py: Python<'_>, max_stacks: usize) -> PyResult<Py<PyAny>> {
        let stacks = self.inner.to_stacks(max_stacks).ok_or_else(|| {
            PyOverflowError::new_err(format!(
                "the GSS represents more than {max_stacks} graph paths; increase max_stacks"
            ))
        })?;
        let result = PyList::empty(py);
        for (values, weight) in stacks {
            let py_values = PyList::new(py, values.into_iter().map(|value| value.0))?;
            let tuple = PyTuple::new(py, [py_values.into_any().unbind(), weight.0])?;
            result.append(tuple)?;
        }
        Ok(result.into_any().unbind())
    }
}

#[pymethods]
impl PyWeightedGSS {
    /// Construct an empty GSS.
    #[new]
    fn new() -> Self {
        Self {
            inner: CoreWeightedGss::empty(),
        }
    }

    /// Construct an empty GSS.
    #[classmethod]
    fn empty(_cls: &Bound<'_, PyType>) -> Self {
        Self::new()
    }

    /// Construct from ``(stack, weight)`` pairs.
    ///
    /// Stacks are ordered bottom-to-top. Duplicate stacks are joined through
    /// the weight's ``join`` method.
    #[classmethod]
    fn from_stacks(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        stacks: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mut converted = Vec::new();
        for item in stacks.try_iter()? {
            let (values, weight): (Vec<Py<PyAny>>, Py<PyAny>) = item?.extract()?;
            for value in &values {
                validate_hashable(py, value, "stack values")?;
            }
            validate_hashable(py, &weight, "weights")?;
            converted.push((values.into_iter().map(PyKey).collect(), PyWeight(weight)));
        }
        Ok(Self {
            inner: CoreWeightedGss::from_stacks(&converted),
        })
    }

    /// Construct from explicit stacks using ``None`` as the weight.
    #[classmethod]
    fn from_unweighted(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        stacks: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let none = py.None();
        let mut converted = Vec::new();
        for item in stacks.try_iter()? {
            let values: Vec<Py<PyAny>> = item?.extract()?;
            for value in &values {
                validate_hashable(py, value, "stack values")?;
            }
            converted.push((
                values.into_iter().map(PyKey).collect(),
                PyWeight(none.clone_ref(py)),
            ));
        }
        Ok(Self {
            inner: CoreWeightedGss::from_stacks(&converted),
        })
    }

    /// Construct a GSS containing one bottom-to-top stack.
    #[classmethod]
    #[pyo3(signature = (stack, weight = None))]
    fn from_single_stack(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        stack: Vec<Py<PyAny>>,
        weight: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        for value in &stack {
            validate_hashable(py, value, "stack values")?;
        }
        let weight = weight.unwrap_or_else(|| py.None());
        validate_hashable(py, &weight, "weights")?;
        Ok(Self {
            inner: CoreWeightedGss::from_single_stack(
                stack.into_iter().map(PyKey).collect(),
                PyWeight(weight),
            ),
        })
    }

    /// Materialize graph paths as ``(stack, weight)`` pairs.
    ///
    /// Raises ``OverflowError`` rather than silently truncating when the
    /// structural path count exceeds ``max_stacks``.
    #[pyo3(signature = (max_stacks = 4096))]
    fn to_stacks(&self, py: Python<'_>, max_stacks: usize) -> PyResult<Py<PyAny>> {
        self.stacks_to_python(py, max_stacks)
    }

    /// Push a value onto every active path.
    fn push(&self, py: Python<'_>, value: Py<PyAny>) -> PyResult<Self> {
        validate_hashable(py, &value, "stack values")?;
        Ok(Self {
            inner: self.inner.push(PyKey(value)),
        })
    }

    /// Pop one value from every non-empty path.
    fn pop(&self) -> Self {
        Self {
            inner: self.inner.pop(),
        }
    }

    /// Pop ``count`` values, discarding paths that underflow.
    fn popn(&self, count: isize) -> PyResult<Self> {
        if count < 0 {
            return Err(PyValueError::new_err("count must be non-negative"));
        }
        Ok(Self {
            inner: self.inner.popn(count),
        })
    }

    /// Keep paths whose top equals ``value``; ``None`` keeps empty paths.
    fn isolate(&self, py: Python<'_>, value: Option<Py<PyAny>>) -> PyResult<Self> {
        if let Some(value) = &value {
            validate_hashable(py, value, "stack values")?;
        }
        Ok(Self {
            inner: self.inner.isolate(value.map(PyKey)),
        })
    }

    /// Return the union with another GSS.
    fn merge(&self, other: &Self) -> Self {
        Self {
            inner: self.inner.merge(&other.inner),
        }
    }

    /// Merge an iterable of GSS values.
    #[classmethod]
    fn merge_many(_cls: &Bound<'_, PyType>, gsses: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut items = Vec::new();
        for item in gsses.try_iter()? {
            let item = item?;
            let gss: PyRef<'_, Self> = item.extract()?;
            items.push(gss.inner.clone());
        }
        Ok(Self {
            inner: CoreWeightedGss::merge_many(items),
        })
    }

    /// Canonicalize multi-depth alternatives.
    ///
    /// ``None`` fuses all levels. This is an advanced structural operation.
    #[pyo3(signature = (levels = None))]
    fn fuse(&self, levels: Option<isize>) -> Self {
        Self {
            inner: self.inner.fuse(levels),
        }
    }

    /// Return the set of values visible at the top of non-empty paths.
    fn peek(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let values: Vec<Py<PyAny>> = self.inner.peek().into_iter().map(|value| value.0).collect();
        Ok(PySet::new(py, &values)?.into_any().unbind())
    }

    /// Join all distinct stored weights, or return ``None`` when empty.
    fn join_weights(&self) -> Option<Py<PyAny>> {
        self.inner.join_weights().map(|weight| weight.0)
    }

    /// Return whether there are no active paths.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Return the maximum represented stack depth.
    fn max_depth(&self) -> u32 {
        self.inner.max_depth()
    }

    /// Count structural graph paths, capped at ``limit``.
    fn path_count_at_most(&self, limit: usize) -> usize {
        self.inner.path_count_at_most(limit)
    }

    /// Return structural graph statistics without materializing paths.
    fn summary(&self) -> PyWeightedGSSSummary {
        self.inner.summary().into()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let summary = self.inner.summary();
        if self.inner.path_count_at_most(17) <= 16 {
            let stacks = self.stacks_to_python(py, 16)?;
            Ok(format!("WeightedGSS({})", stacks.bind(py).repr()?))
        } else {
            Ok(format!(
                "WeightedGSS(paths>16, nodes={}, max_depth={})",
                summary.total_unique_nodes, summary.max_depth
            ))
        }
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyWeightedGSS>()?;
    module.add_class::<PyWeightedGSSSummary>()?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
