use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct Segment<S> {
    data: Arc<Vec<S>>,
    start: usize,
    end: usize,
}

impl<S> Segment<S> {
    pub(crate) fn from_top_first(values: Vec<S>) -> Self {
        let end = values.len();
        Self {
            data: Arc::new(values),
            start: 0,
            end,
        }
    }

    pub(crate) fn one(value: S) -> Self {
        Self::from_top_first(vec![value])
    }

    pub(crate) fn len(&self) -> usize {
        self.end - self.start
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub(crate) fn get(&self, depth: usize) -> Option<&S> {
        self.data
            .get(self.start + depth)
            .filter(|_| depth < self.len())
    }

    pub(crate) fn first(&self) -> Option<&S> {
        self.get(0)
    }

    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &S> + ExactSizeIterator {
        self.data[self.start..self.end].iter()
    }

    pub(crate) fn drop_front(&self, count: usize) -> Option<Self> {
        if count >= self.len() {
            return None;
        }
        Some(Self {
            data: self.data.clone(),
            start: self.start + count,
            end: self.end,
        })
    }
}
