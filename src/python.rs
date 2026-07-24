use std::hash::{Hash, Hasher};

use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyOverflowError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList, PySet, PyTuple, PyType};

use crate::{LeveledGSS as CoreLeveledGSS, LeveledGSSSummary as CoreSummary, Merge};

#[derive(Clone)]
struct PyKey(PyObject);

fn objects_equal(left: &PyObject, right: &PyObject) -> bool {
    Python::with_gil(|py| {
        left.as_ref(py)
            .rich_compare(right.as_ref(py), CompareOp::Eq)
            .and_then(|result| result.is_true())
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
        Python::with_gil(|py| self.0.as_ref(py).hash().unwrap_or(0).hash(state));
    }
}

#[derive(Clone)]
struct PyAccumulator(PyObject);

impl PartialEq for PyAccumulator {
    fn eq(&self, other: &Self) -> bool {
        objects_equal(&self.0, &other.0)
    }
}

impl Eq for PyAccumulator {}

impl Hash for PyAccumulator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Python::with_gil(|py| self.0.as_ref(py).hash().unwrap_or(0).hash(state));
    }
}

impl Merge for PyAccumulator {
    fn merge(&self, other: &Self) -> Self {
        Python::with_gil(|py| {
            if self.0.is_none(py) && other.0.is_none(py) {
                return self.clone();
            }

            match self.0.call_method1(py, "merge", (other.0.clone_ref(py),)) {
                Ok(merged) => Self(merged),
                Err(error) if objects_equal(&self.0, &other.0) => {
                    error.restore(py);
                    let _ = PyErr::take(py);
                    self.clone()
                }
                Err(error) => {
                    error.restore(py);
                    panic!(
                        "Python accumulators must define merge(other), unless both are equal or None"
                    );
                }
            }
        })
    }
}

fn validate_hashable(py: Python<'_>, object: &PyObject, kind: &str) -> PyResult<()> {
    object
        .as_ref(py)
        .hash()
        .map(|_| ())
        .map_err(|_| PyTypeError::new_err(format!("{kind} must be hashable")))
}

#[pyclass(name = "LeveledGSSSummary", module = "leveled_gss._native")]
#[derive(Clone)]
struct PyLeveledGSSSummary {
    #[pyo3(get)]
    top_values_count: usize,
    #[pyo3(get)]
    upperbranch_nodes: usize,
    #[pyo3(get)]
    interface_nodes: usize,
    #[pyo3(get)]
    lower_nodes: usize,
    #[pyo3(get)]
    lower_general_nodes: usize,
    #[pyo3(get)]
    lower_segment_nodes: usize,
    #[pyo3(get)]
    total_unique_nodes: usize,
    #[pyo3(get)]
    total_edges: usize,
    #[pyo3(get)]
    accumulator_instances: usize,
    #[pyo3(get)]
    max_depth: u32,
}

impl From<CoreSummary> for PyLeveledGSSSummary {
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
            accumulator_instances: summary.accumulator_instances,
            max_depth: summary.max_depth,
        }
    }
}

#[pymethods]
impl PyLeveledGSSSummary {
    fn __repr__(&self) -> String {
        format!(
            "LeveledGSSSummary(paths_top_values={}, nodes={}, edges={}, max_depth={})",
            self.top_values_count, self.total_unique_nodes, self.total_edges, self.max_depth
        )
    }
}

#[pyclass(name = "LeveledGSS", module = "leveled_gss._native", unsendable)]
#[derive(Clone)]
struct PyLeveledGSS {
    inner: CoreLeveledGSS<PyKey, PyAccumulator>,
}

impl PyLeveledGSS {
    fn stacks_to_python(&self, py: Python<'_>, max_stacks: usize) -> PyResult<PyObject> {
        let stacks = self.inner.to_stacks(max_stacks).ok_or_else(|| {
            PyOverflowError::new_err(format!(
                "the GSS represents more than {max_stacks} stacks; increase max_stacks"
            ))
        })?;
        let result = PyList::empty(py);
        for (values, accumulator) in stacks {
            let py_values = PyList::new(py, values.into_iter().map(|value| value.0));
            result.append(PyTuple::new(py, [py_values.to_object(py), accumulator.0]))?;
        }
        Ok(result.to_object(py))
    }
}

#[pymethods]
impl PyLeveledGSS {
    #[new]
    fn new() -> Self {
        Self {
            inner: CoreLeveledGSS::empty(),
        }
    }

    #[classmethod]
    fn empty(_cls: &PyType) -> Self {
        Self::new()
    }

    #[classmethod]
    fn from_stacks(_cls: &PyType, py: Python<'_>, stacks: &PyAny) -> PyResult<Self> {
        let mut converted = Vec::new();
        for item in stacks.iter()? {
            let (values, accumulator): (Vec<PyObject>, PyObject) = item?.extract()?;
            for value in &values {
                validate_hashable(py, value, "stack values")?;
            }
            validate_hashable(py, &accumulator, "accumulators")?;
            converted.push((
                values.into_iter().map(PyKey).collect(),
                PyAccumulator(accumulator),
            ));
        }
        Ok(Self {
            inner: CoreLeveledGSS::from_stacks(&converted),
        })
    }

    #[classmethod]
    fn from_unweighted(_cls: &PyType, py: Python<'_>, stacks: &PyAny) -> PyResult<Self> {
        let none = py.None();
        let mut converted = Vec::new();
        for item in stacks.iter()? {
            let values: Vec<PyObject> = item?.extract()?;
            for value in &values {
                validate_hashable(py, value, "stack values")?;
            }
            converted.push((
                values.into_iter().map(PyKey).collect(),
                PyAccumulator(none.clone_ref(py)),
            ));
        }
        Ok(Self {
            inner: CoreLeveledGSS::from_stacks(&converted),
        })
    }

    #[classmethod]
    #[pyo3(signature = (stack, accumulator = None))]
    fn from_single_stack(
        _cls: &PyType,
        py: Python<'_>,
        stack: Vec<PyObject>,
        accumulator: Option<PyObject>,
    ) -> PyResult<Self> {
        for value in &stack {
            validate_hashable(py, value, "stack values")?;
        }
        let accumulator = accumulator.unwrap_or_else(|| py.None());
        validate_hashable(py, &accumulator, "accumulators")?;
        Ok(Self {
            inner: CoreLeveledGSS::from_single_stack(
                stack.into_iter().map(PyKey).collect(),
                PyAccumulator(accumulator),
            ),
        })
    }

    #[pyo3(signature = (max_stacks = 4096))]
    fn to_stacks(&self, py: Python<'_>, max_stacks: usize) -> PyResult<PyObject> {
        self.stacks_to_python(py, max_stacks)
    }

    fn push(&self, py: Python<'_>, value: PyObject) -> PyResult<Self> {
        validate_hashable(py, &value, "stack values")?;
        Ok(Self {
            inner: self.inner.push(PyKey(value)),
        })
    }

    fn pop(&self) -> Self {
        Self {
            inner: self.inner.pop(),
        }
    }

    fn popn(&self, count: isize) -> PyResult<Self> {
        if count < 0 {
            return Err(PyValueError::new_err("count must be non-negative"));
        }
        Ok(Self {
            inner: self.inner.popn(count),
        })
    }

    fn isolate(&self, py: Python<'_>, value: Option<PyObject>) -> PyResult<Self> {
        if let Some(value) = &value {
            validate_hashable(py, value, "stack values")?;
        }
        Ok(Self {
            inner: self.inner.isolate(value.map(PyKey)),
        })
    }

    fn merge(&self, other: &Self) -> Self {
        Self {
            inner: self.inner.merge(&other.inner),
        }
    }

    #[classmethod]
    fn merge_many(_cls: &PyType, gsses: &PyAny) -> PyResult<Self> {
        let mut items = Vec::new();
        for item in gsses.iter()? {
            let item = item?;
            let gss: PyRef<'_, Self> = item.extract()?;
            items.push(gss.inner.clone());
        }
        Ok(Self {
            inner: CoreLeveledGSS::merge_many(items),
        })
    }

    #[pyo3(signature = (levels = None))]
    fn fuse(&self, levels: Option<isize>) -> Self {
        Self {
            inner: self.inner.fuse(levels),
        }
    }

    fn peek(&self, py: Python<'_>) -> PyResult<PyObject> {
        let values: Vec<PyObject> = self.inner.peek().into_iter().map(|value| value.0).collect();
        Ok(PySet::new(py, &values)?.to_object(py))
    }

    fn reduce_acc(&self) -> Option<PyObject> {
        self.inner.reduce_acc().map(|accumulator| accumulator.0)
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn max_depth(&self) -> u32 {
        self.inner.max_depth()
    }

    fn path_count_at_most(&self, limit: usize) -> usize {
        self.inner.path_count_at_most(limit)
    }

    fn summary(&self) -> PyLeveledGSSSummary {
        self.inner.summary().into()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let summary = self.inner.summary();
        if self.inner.path_count_at_most(17) <= 16 {
            let stacks = self.stacks_to_python(py, 16)?;
            Ok(format!("LeveledGSS({})", stacks.as_ref(py).repr()?))
        } else {
            Ok(format!(
                "LeveledGSS(paths>16, nodes={}, max_depth={})",
                summary.total_unique_nodes, summary.max_depth
            ))
        }
    }
}

#[pymodule]
fn _native(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_class::<PyLeveledGSS>()?;
    module.add_class::<PyLeveledGSSSummary>()?;
    Ok(())
}
