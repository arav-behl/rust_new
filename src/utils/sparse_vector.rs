use std::collections::BTreeMap;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct SparseVector<T> {
    data: BTreeMap<usize, T>,
    capacity: usize,
    default_value: Option<T>,
}

impl<T: Clone + Debug> SparseVector<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: BTreeMap::new(),
            capacity,
            default_value: None,
        }
    }

    pub fn with_default(capacity: usize, default: T) -> Self {
        Self {
            data: BTreeMap::new(),
            capacity,
            default_value: Some(default),
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.capacity {
            return None;
        }
        self.data.get(&index).or_else(|| self.default_value.as_ref())
    }

    pub fn set(&mut self, index: usize, value: T) -> Result<Option<T>, &'static str> {
        if index >= self.capacity {
            return Err("Index out of bounds");
        }
        Ok(self.data.insert(index, value))
    }

    pub fn len(&self) -> usize {
        if self.default_value.is_some() {
            self.capacity
        } else {
            self.data.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty() && self.default_value.is_none()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T: Clone + Debug> Default for SparseVector<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_vector_basic() {
        let mut sparse = SparseVector::new(100);
        assert!(sparse.set(10, "test".to_string()).is_ok());
        assert_eq!(sparse.get(10), Some(&"test".to_string()));
    }
}