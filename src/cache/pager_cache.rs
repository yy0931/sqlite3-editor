use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::literal::Literal;

use super::cache_entry::PagerCacheEntry;

pub(super) struct PagerCache {
    cache: Vec<Rc<RefCell<PagerCacheEntry>>>,
}

impl PagerCache {
    pub(super) fn new() -> Self {
        Self { cache: vec![] }
    }

    /// Returns the cache entry that is associated to (query, params).
    /// Inserts an entry if it does not exist.
    pub(super) fn entry(&mut self, query: &str, params: &[Literal]) -> Rc<RefCell<PagerCacheEntry>> {
        let params = &params[0..(params.len() - 2)];
        for entry in &self.cache {
            {
                let entry = entry.borrow();
                if entry.query() != query || entry.params() != params {
                    continue;
                }
            }
            entry.borrow_mut().update_last_accessed();
            return Rc::clone(entry);
        }
        self.cache.push(Rc::new(RefCell::new(PagerCacheEntry::new(
            query.to_owned(),
            params.to_owned(),
            HashMap::new(),
            None,
            None,
        ))));
        Rc::clone(self.cache.last().unwrap())
    }

    /// Returns the total size of the cache in bytes.
    pub(super) fn total_size_bytes(&self) -> u64 {
        self.cache.iter().map(|e| e.borrow().total_size_bytes()).sum::<u64>()
    }

    /// Clears the cache.
    pub(super) fn clear(&mut self) {
        self.cache.clear();
    }

    /// Removes the least recently used entry from the cache.
    pub(super) fn dequeue(&mut self) {
        if let Some((index, _)) = self
            .cache
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.borrow().last_accessed())
        {
            self.cache.remove(index);
        }
    }
}
