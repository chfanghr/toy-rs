use std::{cmp::min, mem};

#[derive(Debug, Clone)]
pub(super) struct Stack<T> {
    storage: Vec<T>,
    height: usize,
}

impl<T> Stack<T> {
    pub(super) fn new() -> Self {
        Self {
            storage: Vec::new(),
            height: 0,
        }
    }

    pub(super) fn height(&self) -> usize {
        self.height
    }

    pub(super) fn available(&self) -> usize {
        self.storage.len()
    }

    pub(super) fn set_height(&mut self, height: usize) {
        assert!(self.available() >= height);
        self.height = height
    }

    pub(super) fn is_empty(&self) -> bool {
        self.height == 0
    }

    pub(super) fn push(&mut self, e: T) {
        let height = self.height;
        if height >= self.storage.len() {
            self.storage.push(e);
        } else {
            self.storage[height] = e;
        }
        self.height += 1;
    }

    pub(super) fn reset(&mut self) {
        self.storage.clear();
        self.height = 0;
    }

    #[allow(dead_code)]
    pub(super) fn trim(&mut self) {
        let mut storage = Vec::new();
        mem::swap(&mut storage, &mut self.storage);
        let storage = storage.into_iter().take(self.height).collect();
        self.storage = storage
    }

    pub(super) fn pop(&mut self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            self.decrease_height_by(1);
            self.storage.get(self.height)
        }
    }

    pub(super) fn pop_cloned(&mut self) -> Option<T>
    where
        T: Clone,
    {
        self.pop().into_iter().cloned().next()
    }

    pub(super) fn pop_n(&mut self, n: usize) -> Vec<&T> {
        let n = min(n, self.height);
        self.decrease_height_by(n);
        // FIXME: use slice
        self.storage
            .iter()
            .skip(self.height)
            .take(n)
            .rev()
            .collect()
    }

    pub(super) fn pop_n_cloned(&mut self, n: usize) -> Vec<T>
    where
        T: Clone,
    {
        self.pop_n(n).into_iter().cloned().collect()
    }

    pub(super) fn peak(&self) -> Option<&T> {
        if self.height == 0 {
            None
        } else {
            Some(self.storage.get(self.height - 1).unwrap())
        }
    }

    #[allow(dead_code)]
    pub(super) fn peak_cloned(&self) -> Option<T>
    where
        T: Clone,
    {
        self.peak().cloned()
    }

    fn decrease_height_by(&mut self, n: usize) {
        assert!(self.height >= n);
        self.height -= n
    }

    pub(super) fn all_available(&self) -> impl Iterator<Item = &T> {
        self.storage.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut stack = Stack::<i64>::new();
        stack.push(1);
        stack.push(2);
        stack.push(3);
        stack.push(4);
        let stack_height = stack.height();
        assert_eq!(stack.pop_cloned(), Some(4));
        assert_eq!(stack.peak_cloned(), Some(3));
        assert_eq!(stack.pop_n_cloned(69), vec![3, 2, 1]);
        stack.set_height(stack_height);
        assert_eq!(stack.peak_cloned(), Some(4));
        stack.pop();
        stack.trim();
        assert_eq!(stack.available(), 3);
    }
}
