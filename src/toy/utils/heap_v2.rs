use std::mem;

use roaring::RoaringBitmap;

const_assert!(size_of::<usize>() >= size_of::<u64>());

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum AddrInternal {
    Null,
    Idx(u32),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum HeapSlot<T> {
    Occupied(T),
    Hole,
}

impl<T> HeapSlot<T> {
    fn new(value: T) -> Self {
        Self::Occupied(value)
    }

    fn get(&self) -> Option<&T> {
        match self {
            HeapSlot::Occupied(value) => Some(value),
            HeapSlot::Hole => None,
        }
    }

    fn get_mut(&mut self) -> Option<&mut T> {
        match self {
            HeapSlot::Occupied(value) => Some(value),
            HeapSlot::Hole => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Addr(AddrInternal);

impl Addr {
    pub fn null() -> Self {
        Addr(AddrInternal::Null)
    }

    fn new_idx(i: u32) -> Self {
        Self(AddrInternal::Idx(i))
    }

    fn must_get_idx(&self) -> u32 {
        match self.0 {
            AddrInternal::Null => panic!("null addr"),
            AddrInternal::Idx(i) => i,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Heap<T> {
    storage: Vec<HeapSlot<T>>,
    holes: RoaringBitmap,
}

impl<T> Heap<T> {
    pub fn new() -> Self {
        Self {
            storage: Vec::new(),
            holes: RoaringBitmap::new(),
        }
    }

    pub fn alloc(&mut self, e: T) -> Addr {
        match self.holes.min() {
            Some(idx) => {
                self.holes.remove(idx);
                self.storage[idx as usize] = HeapSlot::new(e);
                Addr::new_idx(idx)
            }
            None => {
                let idx = self.storage.len();
                self.storage.push(HeapSlot::new(e));
                Addr::new_idx(idx as u32)
            }
        }
    }

    pub fn access(&self, addr: Addr) -> Option<&T> {
        let idx = addr.must_get_idx();
        self.storage[idx as usize].get()
    }

    pub fn access_mut(&mut self, addr: Addr) -> Option<&mut T> {
        let idx = addr.must_get_idx();
        self.storage[idx as usize].get_mut()
    }

    pub fn free(&mut self, addr: Addr) {
        let idx = addr.must_get_idx();
        self.storage[idx as usize] = HeapSlot::Hole;
        self.holes.insert(idx);
    }

    pub fn for_each<F>(&mut self, f: F)
    where
        F: Fn(Addr, T) -> Option<T>,
    {
        self.storage.iter_mut().enumerate().for_each(|(idx, slot)| {
            let slot_in = mem::replace(slot, HeapSlot::Hole);
            if let HeapSlot::Occupied(val) = slot_in
                && let Some(val) = f(Addr::new_idx(idx as u32), val)
            {
                drop(mem::replace(slot, HeapSlot::Occupied(val)));
            }
        });
    }

    pub fn copy(&mut self, from: Addr, to: Addr)
    where
        T: Clone,
    {
        let val = self.access(from).unwrap().clone();
        drop(mem::replace(self.access_mut(to).unwrap(), val));
    }

    pub fn len(&self) -> usize {
        self.storage.len() - (self.holes.len() as usize)
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
        assert_eq!(heap.len(), 3);
        assert_eq!(heap.access(addr_0).copied(), Some(0));
        assert_eq!(heap.access(addr_1).copied(), Some(1));
        assert_eq!(heap.access(addr_2).copied(), Some(2));

        heap.access_mut(addr_2).map(|x| *x = 69);
        assert_eq!(heap.access(addr_2).copied(), Some(69));

        heap.free(addr_2);
        assert_eq!(heap.access(addr_2).copied(), None);
        assert_eq!(heap.access_mut(addr_2).map(|x| *x = 69), None);
        assert_eq!(heap.len(), 2);

        let addr_3 = heap.alloc(42);
        assert_eq!(addr_3, addr_2);
        assert_eq!(heap.access(addr_3).copied(), Some(42))
    }
}
