use bit_set::BitSet;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Addr(usize);

impl Addr {
    fn new(idx: usize) -> Self {
        Self(idx + 1)
    }

    fn un_addr(self) -> usize {
        self.0 - 1
    }

    pub fn null() -> Self {
        Self(0)
    }
}

#[derive(Debug, Clone)]
pub struct Heap<T> {
    storage: Vec<T>,
    holes: BitSet,
}

impl<T> Heap<T> {
    pub fn new() -> Self {
        Self {
            storage: Vec::new(),
            holes: BitSet::new(),
        }
    }

    pub fn addresses(&self) -> impl Iterator<Item = Addr> + 'static {
        let holes = self.holes.clone();
        (0..self.storage.len())
            .filter(move |x| !holes.contains(*x))
            .map(Addr::new)
    }

    pub fn alloc(&mut self, e: T) -> Addr {
        match self.holes.iter().next() {
            Some(idx) => {
                self.storage[idx] = e;
                self.holes.remove(idx);
                Addr::new(idx)
            }
            None => {
                let idx = self.storage.len();
                if idx == usize::MAX {
                    panic!("exceeding max allocation size")
                }
                let addr = Addr::new(idx);
                self.storage.push(e);
                addr
            }
        }
    }

    pub fn free(&mut self, addr: Addr) {
        self.holes.insert(addr.un_addr());
    }

    fn access_check(&self, addr: Addr) -> Option<usize> {
        let idx = addr.un_addr();
        if addr == Addr::null() || idx >= self.storage.len() || self.holes.contains(idx) {
            None
        } else {
            Some(idx)
        }
    }

    pub fn access(&self, addr: Addr) -> Option<&T> {
        self.access_check(addr)
            .map(|idx| self.storage.get(idx).unwrap())
    }

    pub fn access_mut(&mut self, addr: Addr) -> Option<&mut T> {
        self.access_check(addr)
            .map(|idx| self.storage.get_mut(idx).unwrap())
    }

    pub fn size(&self) -> usize {
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
        assert_eq!(heap.access(addr_3).copied(), Some(42))
    }
}
