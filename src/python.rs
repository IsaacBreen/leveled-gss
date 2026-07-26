use crate::{Weight, WeightedGss as CoreWeightedGss};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyOverflowError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList, PyModule, PySet, PyTuple, PyType};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};

thread_local! {
    static PENDING_CALLBACK_ERROR: RefCell<Option<PyErr>> = const { RefCell::new(None) };
}

fn record_callback_error(error: PyErr) {
    PENDING_CALLBACK_ERROR.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    });
}

fn callback_error_pending() -> bool {
    PENDING_CALLBACK_ERROR.with(|slot| slot.borrow().is_some())
}

fn run_callbacks<T>(operation: impl FnOnce() -> T) -> PyResult<T> {
    PENDING_CALLBACK_ERROR.with(|slot| {
        slot.borrow_mut().take();
    });
    let result = operation();
    match PENDING_CALLBACK_ERROR.with(|slot| slot.borrow_mut().take()) {
        Some(error) => Err(error),
        None => Ok(result),
    }
}

struct PyKey {
    object: Py<PyAny>,
    hash: isize,
}

impl PyKey {
    fn new(py: Python<'_>, object: Py<PyAny>) -> PyResult<Self> {
        let hash = object
            .bind(py)
            .hash()
            .map_err(|_| PyTypeError::new_err("stack values must be hashable"))?;
        Ok(Self { object, hash })
    }
}

impl Clone for PyKey {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            object: self.object.clone_ref(py),
            hash: self.hash,
        })
    }
}

impl PartialEq for PyKey {
    fn eq(&self, other: &Self) -> bool {
        Python::attach(|py| {
            if self.object.bind(py).is(other.object.bind(py)) {
                return true;
            }
            match self
                .object
                .bind(py)
                .rich_compare(other.object.bind(py), CompareOp::Eq)
                .and_then(|result| result.is_truthy())
            {
                Ok(equal) => equal,
                Err(error) => {
                    record_callback_error(error);
                    false
                }
            }
        })
    }
}

impl Eq for PyKey {}

impl Hash for PyKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

struct PyWeight(Py<PyAny>);

impl PyWeight {
    fn new(py: Python<'_>, object: Py<PyAny>) -> PyResult<Self> {
        if !object.bind(py).is_none() && !object.bind(py).hasattr("join")? {
            return Err(PyTypeError::new_err(
                "weights must be None or provide join(other)",
            ));
        }
        Ok(Self(object))
    }
}

impl Clone for PyWeight {
    fn clone(&self) -> Self {
        Python::attach(|py| Self(self.0.clone_ref(py)))
    }
}

impl PartialEq for PyWeight {
    fn eq(&self, other: &Self) -> bool {
        Python::attach(|py| self.0.bind(py).is(other.0.bind(py)))
    }
}

impl Weight for PyWeight {
    fn join(&self, other: &Self) -> Self {
        Python::attach(|py| {
            if self.0.bind(py).is_none() && other.0.bind(py).is_none() {
                return Self(py.None());
            }
            if callback_error_pending() {
                return self.clone();
            }
            match self.0.call_method1(py, "join", (other.0.clone_ref(py),)) {
                Ok(joined) => Self(joined),
                Err(error) => {
                    record_callback_error(error);
                    self.clone()
                }
            }
        })
    }
}

/// A persistent collection of weighted stack alternatives.
///
/// Stacks are ordered bottom-to-top. Stack values must be immutable and
/// hashable. Weights may be ``None`` or objects whose ``join(other)`` method is
/// associative, commutative, and idempotent.
#[pyclass(
    name = "WeightedGSS",
    module = "weighted_gss._native",
    unsendable,
    skip_from_py_object
)]
struct PyWeightedGss {
    inner: CoreWeightedGss<PyKey, PyWeight>,
}

impl Clone for PyWeightedGss {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl PyWeightedGss {
    fn convert_stack(py: Python<'_>, stack: Vec<Py<PyAny>>) -> PyResult<Vec<PyKey>> {
        stack
            .into_iter()
            .map(|value| PyKey::new(py, value))
            .collect()
    }

    fn to_python_stacks(&self, py: Python<'_>, max_paths: usize) -> PyResult<Py<PyAny>> {
        let stacks = run_callbacks(|| self.inner.to_stacks(max_paths))?.map_err(|_| {
            PyOverflowError::new_err(format!(
                "the GSS contains more than {max_paths} structural paths; increase max_paths"
            ))
        })?;
        let result = PyList::empty(py);
        for (stack, weight) in stacks {
            let values = PyList::new(py, stack.into_iter().map(|value| value.object))?;
            let pair = PyTuple::new(py, [values.into_any().unbind(), weight.0])?;
            result.append(pair)?;
        }
        Ok(result.into_any().unbind())
    }
}

#[pymethods]
impl PyWeightedGss {
    /// Construct an empty weighted GSS.
    #[new]
    fn new() -> Self {
        Self {
            inner: CoreWeightedGss::new(),
        }
    }

    /// Construct one bottom-to-top stack.
    #[classmethod]
    #[pyo3(signature = (stack, weight = None))]
    fn from_stack(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        stack: Vec<Py<PyAny>>,
        weight: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let stack = Self::convert_stack(py, stack)?;
        let weight = PyWeight::new(py, weight.unwrap_or_else(|| py.None()))?;
        Ok(Self {
            inner: run_callbacks(|| CoreWeightedGss::from_stack(stack, weight))?,
        })
    }

    /// Construct from ``(stack, weight)`` pairs.
    #[classmethod]
    fn from_stacks(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        entries: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mut converted = Vec::new();
        for entry in entries.try_iter()? {
            let (stack, weight): (Vec<Py<PyAny>>, Py<PyAny>) = entry?.extract()?;
            converted.push((Self::convert_stack(py, stack)?, PyWeight::new(py, weight)?));
        }
        Ok(Self {
            inner: run_callbacks(|| CoreWeightedGss::from_stacks(converted))?,
        })
    }

    /// Construct unweighted stacks, represented by the shared weight ``None``.
    #[classmethod]
    fn from_unweighted(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        stacks: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mut converted = Vec::new();
        for stack in stacks.try_iter()? {
            converted.push(Self::convert_stack(py, stack?.extract()?)?);
        }
        let weight = PyWeight(py.None());
        Ok(Self {
            inner: run_callbacks(|| CoreWeightedGss::from_stacks_with_weight(converted, weight))?,
        })
    }

    /// Return a new value containing the existing alternatives plus ``stack``.
    #[pyo3(signature = (stack, weight = None))]
    fn with_stack(
        &self,
        py: Python<'_>,
        stack: Vec<Py<PyAny>>,
        weight: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let stack = Self::convert_stack(py, stack)?;
        let weight = PyWeight::new(py, weight.unwrap_or_else(|| py.None()))?;
        Ok(Self {
            inner: run_callbacks(|| self.inner.with_stack(stack, weight))?,
        })
    }

    /// Merge another weighted GSS into this one.
    fn merge(&self, other: &Self) -> PyResult<Self> {
        Ok(Self {
            inner: run_callbacks(|| self.inner.merge(&other.inner))?,
        })
    }

    /// Merge an iterable of weighted GSS values.
    #[classmethod]
    fn merge_all(_cls: &Bound<'_, PyType>, values: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut converted = Vec::new();
        for value in values.try_iter()? {
            let value = value?;
            let value: PyRef<'_, Self> = value.extract()?;
            converted.push(value.inner.clone());
        }
        Ok(Self {
            inner: run_callbacks(|| CoreWeightedGss::merge_all(converted))?,
        })
    }

    /// Push ``value`` onto every represented stack.
    fn push(&self, py: Python<'_>, value: Py<PyAny>) -> PyResult<Self> {
        let value = PyKey::new(py, value)?;
        Ok(Self {
            inner: run_callbacks(|| self.inner.push(value))?,
        })
    }

    /// Pop one value, discarding empty alternatives.
    fn pop(&self) -> PyResult<Self> {
        Ok(Self {
            inner: run_callbacks(|| self.inner.pop())?,
        })
    }

    /// Pop ``count`` values, discarding alternatives that underflow.
    fn popn(&self, count: isize) -> PyResult<Self> {
        let count = usize::try_from(count)
            .map_err(|_| PyValueError::new_err("count must be non-negative"))?;
        Ok(Self {
            inner: run_callbacks(|| self.inner.popn(count))?,
        })
    }

    /// Return the unique non-empty top value.
    ///
    /// Raises ``ValueError`` when the GSS is empty, has multiple possible tops,
    /// or also contains an empty-stack alternative.
    fn top(&self) -> PyResult<Py<PyAny>> {
        run_callbacks(|| self.inner.top())?
            .map(|value| value.object)
            .ok_or_else(|| PyValueError::new_err("the GSS does not have one exclusive top value"))
    }

    /// Return the distinct non-empty top values.
    fn tops(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let tops = run_callbacks(|| self.inner.tops().collect::<Vec<_>>())?;
        let values = tops
            .into_iter()
            .map(|value| value.object)
            .collect::<Vec<_>>();
        Ok(PySet::new(py, &values)?.into_any().unbind())
    }

    /// Return whether an empty-stack alternative is present.
    fn has_empty_stack(&self) -> bool {
        self.inner.has_empty_stack()
    }

    /// Retain alternatives whose top equals ``value`` without popping it.
    fn retain_top(&self, py: Python<'_>, value: Py<PyAny>) -> PyResult<Self> {
        let value = PyKey::new(py, value)?;
        Ok(Self {
            inner: run_callbacks(|| self.inner.retain_top(&value))?,
        })
    }

    /// Retain only empty-stack alternatives.
    fn retain_empty(&self) -> PyResult<Self> {
        Ok(Self {
            inner: run_callbacks(|| self.inner.retain_empty())?,
        })
    }

    /// Retain alternatives with matching top ``value`` and pop that top.
    fn pop_top(&self, py: Python<'_>, value: Py<PyAny>) -> PyResult<Self> {
        let value = PyKey::new(py, value)?;
        Ok(Self {
            inner: run_callbacks(|| self.inner.pop_top(&value))?,
        })
    }

    /// Return ``(top, remainder)`` pairs for every non-empty top branch.
    fn pop_branches(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let branches = run_callbacks(|| self.inner.pop_branches().collect::<Vec<_>>())?;
        let result = PyList::empty(py);
        for branch in branches {
            let remainder = Py::new(
                py,
                Self {
                    inner: branch.remainder,
                },
            )?;
            let pair = PyTuple::new(py, [branch.top.object, remainder.into_any()])?;
            result.append(pair)?;
        }
        Ok(result.into_any().unbind())
    }

    /// Join every represented path weight.
    ///
    /// Raises ``ValueError`` when the GSS is empty. The returned weight may
    /// itself be ``None`` for an unweighted GSS.
    fn joined_weight(&self) -> PyResult<Py<PyAny>> {
        run_callbacks(|| self.inner.joined_weight())?
            .map(|weight| weight.0)
            .ok_or_else(|| PyValueError::new_err("the GSS is empty"))
    }

    /// Return the joined weight of the empty stack.
    ///
    /// Raises ``ValueError`` when no empty-stack alternative exists. The
    /// returned weight may itself be ``None`` for an unweighted GSS.
    fn empty_weight(&self) -> PyResult<Py<PyAny>> {
        run_callbacks(|| self.inner.empty_weight())?
            .map(|weight| weight.0)
            .ok_or_else(|| PyValueError::new_err("the GSS has no empty-stack alternative"))
    }

    /// Materialize extensional ``(stack, weight)`` pairs.
    ///
    /// Raises ``OverflowError`` instead of silently truncating when more than
    /// ``max_paths`` structural paths would be traversed.
    #[pyo3(signature = (max_paths = 4096))]
    fn to_stacks(&self, py: Python<'_>, max_paths: usize) -> PyResult<Py<PyAny>> {
        self.to_python_stacks(py, max_paths)
    }

    /// Return whether no alternatives are represented.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Return the maximum represented stack depth.
    fn max_depth(&self) -> usize {
        self.inner.max_depth()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        let paths = self.inner.paths().path_count_at_most(17);
        if paths <= 16 {
            format!(
                "WeightedGSS(paths={paths}, max_depth={})",
                self.inner.max_depth()
            )
        } else {
            format!(
                "WeightedGSS(paths>16, max_depth={})",
                self.inner.max_depth()
            )
        }
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyWeightedGss>()?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
