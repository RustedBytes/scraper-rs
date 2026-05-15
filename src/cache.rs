use std::collections::{HashMap, VecDeque, hash_map::Entry};

pub(crate) struct FixedCache<T> {
    map: HashMap<String, T>,
    order: VecDeque<String>,
    capacity: usize,
}

impl<T> FixedCache<T> {
    #[inline]
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    #[inline]
    pub(crate) fn get(&self, key: &str) -> Option<&T> {
        self.map.get(key)
    }

    pub(crate) fn insert(&mut self, key: String, value: T) {
        let key = match self.map.entry(key) {
            Entry::Occupied(mut entry) => {
                entry.insert(value);
                return;
            }
            Entry::Vacant(entry) => entry.into_key(),
        };

        if self.map.len() >= self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.map.remove(&oldest);
        }

        self.order.push_back(key.clone());
        self.map.insert(key, value);
    }
}
