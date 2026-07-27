use crate::nodes::{UKind, URef, WKind, WRef, u_id, w_id};
use crate::{Weight, WeightedGss as CoreWeightedGss};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyOverflowError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PyModule, PySet, PyTuple, PyType};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

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

fn enqueue_weighted_node(
    node: &WRef<PyKey, PyWeight>,
    ids: &mut FxHashMap<usize, usize>,
    queue: &mut VecDeque<WRef<PyKey, PyWeight>>,
) -> usize {
    let pointer = w_id(node);
    if let Some(id) = ids.get(&pointer) {
        return *id;
    }
    let id = ids.len();
    ids.insert(pointer, id);
    queue.push_back(node.clone());
    id
}

fn enqueue_unweighted_node(
    node: &URef<PyKey>,
    ids: &mut FxHashMap<usize, usize>,
    queue: &mut VecDeque<URef<PyKey>>,
) -> usize {
    let pointer = u_id(node);
    if let Some(id) = ids.get(&pointer) {
        return *id;
    }
    let id = ids.len();
    ids.insert(pointer, id);
    queue.push_back(node.clone());
    id
}

fn weight_reference(
    py: Python<'_>,
    weight: &Arc<PyWeight>,
    ids: &mut FxHashMap<usize, usize>,
    output: &Bound<'_, PyList>,
) -> PyResult<String> {
    let pointer = Arc::as_ptr(weight) as usize;
    if let Some(id) = ids.get(&pointer) {
        return Ok(format!("a{id}"));
    }
    let id = ids.len();
    ids.insert(pointer, id);
    let entry = PyDict::new(py);
    entry.set_item("id", format!("a{id}"))?;
    entry.set_item("value", weight.0.clone_ref(py))?;
    output.append(entry)?;
    Ok(format!("a{id}"))
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

    fn to_python_stacks(&self, py: Python<'_>, max_stacks: usize) -> PyResult<Py<PyAny>> {
        let stacks = run_callbacks(|| self.inner.to_stacks(max_stacks))?.map_err(|_| {
            PyOverflowError::new_err(format!(
                "the GSS contains more than {max_stacks} distinct stacks; increase max_stacks"
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

    fn dump_structure(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let result = PyDict::new(py);
        let nodes = PyList::empty(py);
        let edges = PyList::empty(py);
        let weights = PyList::empty(py);

        let mut weighted_ids = FxHashMap::default();
        let mut unweighted_ids = FxHashMap::default();
        let mut weight_ids = FxHashMap::default();
        let mut weighted_queue = VecDeque::new();
        let mut unweighted_queue = VecDeque::new();

        let root_id =
            enqueue_weighted_node(&self.inner.root, &mut weighted_ids, &mut weighted_queue);

        while let Some(node) = weighted_queue.pop_front() {
            let source_id = weighted_ids[&w_id(&node)];
            let node_output = PyDict::new(py);
            node_output.set_item("id", format!("w{source_id}"))?;
            node_output.set_item("enum", "WKind")?;
            node_output.set_item("layer", "weighted")?;
            node_output.set_item("paths", node.paths)?;
            node_output.set_item("max_depth", node.max_depth)?;

            match &node.kind {
                WKind::Branch { empty, children } => {
                    node_output.set_item("variant", "Branch")?;
                    let empty_weights = PyList::empty(py);
                    for weight in empty {
                        empty_weights.append(weight_reference(
                            py,
                            weight,
                            &mut weight_ids,
                            &weights,
                        )?)?;
                    }
                    node_output.set_item("empty_weights", empty_weights)?;

                    for (value, alternatives) in children {
                        for (alternative, child) in alternatives.iter().enumerate() {
                            let target_id = enqueue_weighted_node(
                                child,
                                &mut weighted_ids,
                                &mut weighted_queue,
                            );
                            let edge = PyDict::new(py);
                            edge.set_item("from", format!("w{source_id}"))?;
                            edge.set_item("to", format!("w{target_id}"))?;
                            edge.set_item("kind", "stack")?;
                            edge.set_item("value", value.object.clone_ref(py))?;
                            edge.set_item("alternative", alternative)?;
                            edges.append(edge)?;
                        }
                    }
                }
                WKind::Shared { weight, stacks } => {
                    node_output.set_item("variant", "Shared")?;
                    node_output.set_item(
                        "weight",
                        weight_reference(py, weight, &mut weight_ids, &weights)?,
                    )?;
                    let target_id =
                        enqueue_unweighted_node(stacks, &mut unweighted_ids, &mut unweighted_queue);
                    let edge = PyDict::new(py);
                    edge.set_item("from", format!("w{source_id}"))?;
                    edge.set_item("to", format!("u{target_id}"))?;
                    edge.set_item("kind", "shared_stacks")?;
                    edges.append(edge)?;
                }
            }
            nodes.append(node_output)?;
        }

        while let Some(node) = unweighted_queue.pop_front() {
            let source_id = unweighted_ids[&u_id(&node)];
            let node_output = PyDict::new(py);
            node_output.set_item("id", format!("u{source_id}"))?;
            node_output.set_item("enum", "UKind")?;
            node_output.set_item("layer", "unweighted")?;
            node_output.set_item("paths", node.paths)?;
            node_output.set_item("max_depth", node.max_depth)?;

            match &node.kind {
                UKind::Branch { empty, children } => {
                    node_output.set_item("variant", "Branch")?;
                    node_output.set_item("empty", *empty)?;
                    for (value, alternatives) in children {
                        for (alternative, child) in alternatives.iter().enumerate() {
                            let target_id = enqueue_unweighted_node(
                                child,
                                &mut unweighted_ids,
                                &mut unweighted_queue,
                            );
                            let edge = PyDict::new(py);
                            edge.set_item("from", format!("u{source_id}"))?;
                            edge.set_item("to", format!("u{target_id}"))?;
                            edge.set_item("kind", "stack")?;
                            edge.set_item("value", value.object.clone_ref(py))?;
                            edge.set_item("alternative", alternative)?;
                            edges.append(edge)?;
                        }
                    }
                }
                UKind::Segment { values, next } => {
                    node_output.set_item("variant", "Segment")?;
                    let segment_values =
                        PyList::new(py, values.iter().map(|value| value.object.clone_ref(py)))?;
                    node_output.set_item("values_top_first", segment_values)?;
                    let target_id =
                        enqueue_unweighted_node(next, &mut unweighted_ids, &mut unweighted_queue);
                    let edge = PyDict::new(py);
                    edge.set_item("from", format!("u{source_id}"))?;
                    edge.set_item("to", format!("u{target_id}"))?;
                    edge.set_item("kind", "segment_next")?;
                    edges.append(edge)?;
                }
            }
            nodes.append(node_output)?;
        }

        result.set_item("schema", "weighted-gss/internal-structure/v1")?;
        result.set_item("root", format!("w{root_id}"))?;
        result.set_item("nodes", nodes)?;
        result.set_item("edges", edges)?;
        result.set_item("weights", weights)?;
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
        let branches = run_callbacks(|| self.inner.pop_branches())?;
        let result = PyList::empty(py);
        for (top, remainder) in branches {
            let remainder = Py::new(py, Self { inner: remainder })?;
            let pair = PyTuple::new(py, [top.object, remainder.into_any()])?;
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
    /// ``max_stacks`` distinct stacks would be materialized.
    #[pyo3(signature = (max_stacks = 4096))]
    fn to_stacks(&self, py: Python<'_>, max_stacks: usize) -> PyResult<Py<PyAny>> {
        self.to_python_stacks(py, max_stacks)
    }

    /// Return whether no alternatives are represented.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Return the maximum represented stack depth.
    fn max_depth(&self) -> usize {
        self.inner.max_depth()
    }

    /// Return the internal shared graph as Python dictionaries and lists.
    ///
    /// This private diagnostic method exposes implementation details and may
    /// change without notice.
    fn _dump_structure(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.dump_structure(py)
    }

    /// Return the internal shared graph as JSON.
    ///
    /// Values that are not directly JSON serializable are represented with
    /// ``repr``. This private diagnostic format may change without notice.
    #[pyo3(signature = (indent = 2))]
    fn _dump_json(&self, py: Python<'_>, indent: usize) -> PyResult<String> {
        let structure = self.dump_structure(py)?;
        let json = PyModule::import(py, "json")?;
        let builtins = PyModule::import(py, "builtins")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("indent", indent)?;
        kwargs.set_item("default", builtins.getattr("repr")?)?;
        json.call_method("dumps", (structure,), Some(&kwargs))?
            .extract()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!(
            "WeightedGSS(is_empty={}, max_depth={})",
            self.inner.is_empty(),
            self.inner.max_depth()
        )
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyWeightedGss>()?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
