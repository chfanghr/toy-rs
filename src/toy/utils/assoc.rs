use std::{borrow::Borrow, collections::BTreeMap, iter};

#[derive(Debug, Clone)]
pub struct Assoc<K, V>(Vec<BTreeMap<K, V>>);

impl<K: Ord, V> Assoc<K, V> {
    pub fn new() -> Assoc<K, V> {
        Assoc(vec![BTreeMap::new()])
    }

    pub fn insert(&mut self, k: K, v: V) {
        let _ = self.0.first_mut().unwrap().insert(k, v);
    }

    pub fn lookup<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.0.iter().find_map(|b| b.get(k))
    }

    // Right-bias
    pub fn combine(l: Assoc<K, V>, r: Assoc<K, V>) -> Assoc<K, V> {
        Assoc(r.0.into_iter().chain(l.0).collect())
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0
            .iter()
            .rev()
            .flat_map(|m| m.iter())
            .collect::<BTreeMap<&K, &V>>()
            .into_iter()
            .map(|(_, v)| v)
    }

    pub fn size(&self) -> usize {
        self.values().count()
    }
}

impl<K, V> iter::FromIterator<(K, V)> for Assoc<K, V>
where
    K: Ord,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Assoc(vec![iter.into_iter().collect()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
