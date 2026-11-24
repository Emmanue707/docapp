use std::collections::HashMap;

use crate::tile::{Tile, TileKey};

struct Node {
    prev: Option<TileKey>,
    next: Option<TileKey>,
}

pub struct TileCache {
    budget: usize,
    used: usize,
    map: HashMap<TileKey, (Tile, Node)>,
    head: Option<TileKey>,
    tail: Option<TileKey>,
}

impl TileCache {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget: budget_bytes,
            used: 0,
            map: HashMap::new(),
            head: None,
            tail: None,
        }
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn used_bytes(&self) -> usize {
        self.used
    }

    pub fn get(&mut self, key: &TileKey) -> Option<Tile> {
        if !self.map.contains_key(key) {
            return None;
        }
        self.detach(key);
        self.attach_front(key);
        self.map.get(key).map(|(t, _)| t.clone())
    }

    pub fn peek(&self, key: &TileKey) -> Option<&Tile> {
        self.map.get(key).map(|(t, _)| t)
    }

    pub fn put(&mut self, key: TileKey, tile: Tile) {
        let bytes = tile.byte_len();
        if bytes > self.budget {
            return;
        }
        if let Some((old, _)) = self.map.remove(&key) {
            self.used = self.used.saturating_sub(old.byte_len());
            self.unlink(&key);
        }
        self.evict_until(bytes);
        self.map.insert(
            key,
            (
                tile,
                Node {
                    prev: None,
                    next: None,
                },
            ),
        );
        self.used += bytes;
        self.attach_front(&key);
    }

    fn evict_until(&mut self, incoming: usize) {
        while self.used + incoming > self.budget {
            let Some(tail) = self.tail else {
                break;
            };
            if let Some((tile, _)) = self.map.remove(&tail) {
                self.used = self.used.saturating_sub(tile.byte_len());
            }
            self.unlink(&tail);
        }
    }

    fn detach(&mut self, key: &TileKey) {
        self.unlink(key);
    }

    fn unlink(&mut self, key: &TileKey) {
        let (prev, next) = match self.map.get(key) {
            Some((_, node)) => (node.prev, node.next),
            None => return,
        };
        match prev {
            Some(p) => {
                if let Some((_, n)) = self.map.get_mut(&p) {
                    n.next = next;
                }
            }
            None => self.head = next,
        }
        match next {
            Some(n) => {
                if let Some((_, nn)) = self.map.get_mut(&n) {
                    nn.prev = prev;
                }
            }
            None => self.tail = prev,
        }
        if let Some((_, node)) = self.map.get_mut(key) {
            node.prev = None;
            node.next = None;
        }
    }

    fn attach_front(&mut self, key: &TileKey) {
        let old_head = self.head;
        if let Some((_, node)) = self.map.get_mut(key) {
            node.prev = None;
            node.next = old_head;
        }
        if let Some(h) = old_head
            && let Some((_, n)) = self.map.get_mut(&h)
        {
            n.prev = Some(*key);
        }
        self.head = Some(*key);
        if self.tail.is_none() {
            self.tail = Some(*key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(size: u32) -> Tile {
        let n = (size * size * 4) as usize;
        Tile {
            width: size,
            height: size,
            bytes: vec![0u8; n],
        }
    }

    #[test]
    fn lru_evicts_by_bytes() {
        let mut c = TileCache::new(1000);
        c.put(
            TileKey {
                doc_id: 1,
                page: 0,
                zoom_bucket: 0,
                tx: 0,
                ty: 0,
            },
            tile(10),
        );
        c.put(
            TileKey {
                doc_id: 1,
                page: 0,
                zoom_bucket: 0,
                tx: 1,
                ty: 0,
            },
            tile(10),
        );
        assert!(c.used_bytes() <= 1000);
        c.put(
            TileKey {
                doc_id: 1,
                page: 0,
                zoom_bucket: 0,
                tx: 2,
                ty: 0,
            },
            tile(11),
        );
        assert!(c.used_bytes() <= 1000);
        let k0 = TileKey {
            doc_id: 1,
            page: 0,
            zoom_bucket: 0,
            tx: 0,
            ty: 0,
        };
        assert!(c.peek(&k0).is_none());
    }

    #[test]
    fn get_moves_to_mru() {
        let mut c = TileCache::new(900);
        let k0 = TileKey {
            doc_id: 1,
            page: 0,
            zoom_bucket: 0,
            tx: 0,
            ty: 0,
        };
        let k1 = TileKey {
            doc_id: 1,
            page: 0,
            zoom_bucket: 0,
            tx: 1,
            ty: 0,
        };
        c.put(k0, tile(10));
        c.put(k1, tile(10));
        let _ = c.get(&k0);
        c.put(
            TileKey {
                doc_id: 1,
                page: 0,
                zoom_bucket: 0,
                tx: 2,
                ty: 0,
            },
            tile(11),
        );
        assert!(c.peek(&k0).is_some());
        assert!(c.peek(&k1).is_none());
    }
}
