use hashlink::LinkedHashMap;
use std::hash::Hash;

#[derive(Debug)]
pub struct ByteLruCache<K, V> {
    max_entries: usize,
    max_bytes: usize,
    bytes: usize,
    map: LinkedHashMap<K, (V, usize)>,
}

impl<K: Eq + Hash, V> ByteLruCache<K, V> {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            bytes: 0,
            map: LinkedHashMap::new(),
        }
    }

    pub fn get(&mut self, key: &K) -> Option<V>
    where
        K: Clone,
        V: Clone,
    {
        let (v, sz) = self.map.remove(key)?;
        let out = v.clone();
        self.map.insert(key.clone(), (v, sz));
        Some(out)
    }

    pub fn insert(&mut self, key: K, value: V, size_bytes: usize) {
        let size_bytes = size_bytes.max(1);

        if let Some((_old_v, old_sz)) = self.map.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old_sz);
        }

        self.map.insert(key, (value, size_bytes));
        self.bytes = self.bytes.saturating_add(size_bytes);
        self.evict();
    }

    fn evict(&mut self) {
        while self.map.len() > self.max_entries || self.bytes > self.max_bytes {
            if let Some((_k, (_v, sz))) = self.map.pop_front() {
                self.bytes = self.bytes.saturating_sub(sz);
                continue;
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ByteLruCache;

    #[test]
    fn evicts_by_entries() {
        let mut c = ByteLruCache::new(2, 100);
        c.insert(1, "a".to_string(), 1);
        c.insert(2, "b".to_string(), 1);
        c.insert(3, "c".to_string(), 1);
        assert!(c.get(&1).is_none());
        assert_eq!(c.get(&2).as_deref(), Some("b"));
        assert_eq!(c.get(&3).as_deref(), Some("c"));
    }

    #[test]
    fn evicts_by_bytes() {
        let mut c = ByteLruCache::new(10, 3);
        c.insert(1, "a".to_string(), 2);
        c.insert(2, "b".to_string(), 2);
        assert!(c.get(&1).is_none());
        assert_eq!(c.get(&2).as_deref(), Some("b"));
    }

    #[test]
    fn refresh_on_get_keeps_recent() {
        let mut c = ByteLruCache::new(2, 100);
        c.insert(1, "a".to_string(), 1);
        c.insert(2, "b".to_string(), 1);
        let _ = c.get(&1);
        c.insert(3, "c".to_string(), 1);
        assert!(c.get(&2).is_none());
        assert_eq!(c.get(&1).as_deref(), Some("a"));
    }
}
