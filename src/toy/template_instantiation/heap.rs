use bit_set::BitSet;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Addr(usize);

#[derive(Debug, Clone)]
pub(super) struct Heap<T> {
    storage: Vec<T>,
    holes: BitSet,
}

impl<T> Heap<T> {
    pub(super) fn new() -> Self {
        Self {
            storage: Vec::new(),
            holes: BitSet::new(),
        }
    }

    pub(super) fn addresses(&self) -> impl Iterator<Item = Addr> {
        let holes = self.holes.clone();
        (0..self.storage.len())
            .filter(move |x| !holes.contains(*x))
            .map(Addr)
    }

    pub(super) fn alloc(&mut self, e: T) -> Addr {
        match self.holes.iter().next() {
            Some(addr) => {
                self.storage[addr] = e;
                Addr(addr)
            }
            None => {
                let addr = Addr(self.storage.len());
                self.storage.push(e);
                addr
            }
        }
    }

    pub(super) fn free(&mut self, addr: Addr) {
        self.holes.insert(addr.0);
    }

    pub(super) fn access(&self, addr: Addr) -> Option<&T> {
        if addr.0 >= self.storage.len() || self.holes.contains(addr.0) {
            None
        } else {
            self.storage.get(addr.0)
        }
    }

    pub(super) fn access_mut(&mut self, addr: Addr) -> Option<&mut T> {
        if addr.0 >= self.storage.len() || self.holes.contains(addr.0) {
            None
        } else {
            self.storage.get_mut(addr.0)
        }
    }

    pub(super) fn size(&self) -> usize {
        self.storage.len() - self.holes.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut heap = Heap::<i64>::new();
        let addr_0 = heap.alloc(0);
        let addr_1 = heap.alloc(1);
        let addr_2 = heap.alloc(2);
        assert_eq!(heap.size(), 3);
        assert_eq!(heap.access(addr_0).copied(), Some(0));
        assert_eq!(heap.access(addr_1).copied(), Some(1));
        assert_eq!(heap.access(addr_2).copied(), Some(2));

        heap.access_mut(addr_2).map(|x| *x = 69);
        assert_eq!(heap.access(addr_2).copied(), Some(69));

        heap.free(addr_2);
        assert_eq!(heap.access(addr_2).copied(), None);
        assert_eq!(heap.access_mut(addr_2).map(|x| *x = 69), None);
        assert_eq!(heap.size(), 2);

        let addr_3 = heap.alloc(42);
        assert_eq!(addr_3, addr_2);
    }
}
