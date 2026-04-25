use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(super) struct Assoc<K, V>(Vec<BTreeMap<K, V>>);

impl<K: Ord, V> Assoc<K, V> {
    pub(super) fn new() -> Assoc<K, V> {
        Assoc(vec![BTreeMap::new()])
    }

    pub(super) fn insert(&mut self, k: K, v: V) {
        let _ = self.0.first_mut().unwrap().insert(k, v);
    }

    pub(super) fn lookup(&self, k: &K) -> Option<&V> {
        self.0.iter().find_map(|b| b.get(k))
    }

    // Right-bias
    pub(super) fn combine(l: Assoc<K, V>, r: Assoc<K, V>) -> Assoc<K, V> {
        Assoc(r.0.into_iter().chain(l.0).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::template_instantiation::assoc::Assoc;

    #[test]
    fn test() {
        let mut assoc_a = Assoc::<usize, usize>::new();
        let mut assoc_b = Assoc::<usize, usize>::new();

        assoc_a.insert(0, 0);
        assoc_a.insert(1, 0);

        assoc_b.insert(0, 69);

        let assoc_combined = Assoc::combine(assoc_a, assoc_b);

        assert_eq!(assoc_combined.lookup(&0).copied(), Some(69));
        assert_eq!(assoc_combined.lookup(&1).copied(), Some(0));
    }
}
