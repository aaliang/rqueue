use std::{ptr, mem};
use std::hash::{SipHasher, Hash, Hasher};

/// size of static array
const STATIC_SZ: usize = 2048;

enum InlineVec <T> {
    Static(usize, [Vec<T>; STATIC_SZ]), //this is kind of silly, probably should just use vecs
    Dynamic(Vec<Vec<T>>)
}

/// A specialized version of a HashMap
/// lookups can be done on byte slices without allocations
/// keys are stored as vectors
/// collisions are handled by chaining on vectors
pub struct SliceMap <V> {
    count: usize,
    table: InlineVec<HashEntry<V>>,
    capacity: usize
}

//TODO: handle resizing

impl <V> SliceMap <V> {
    pub fn new () -> SliceMap <V> {
        let stat = unsafe {
            let mut stat:[Vec<HashEntry<V>>; STATIC_SZ] = mem::uninitialized();
            for i in stat.iter_mut() {
                ptr::write(i, Vec::new());
            }
            stat
        };
        SliceMap {
            count: 0,
            table: InlineVec::Static(0, stat),
            capacity: STATIC_SZ
        }
    }

    /// inserts a value to the map
    pub fn insert (&mut self, key: &[u8], val: V) {
        let mut tab = match self.table {
            InlineVec::Static(_, ref mut arr) => &mut arr[..],
            InlineVec::Dynamic(ref mut vec) => &mut vec[..]
        };

        let hash = Self::make_hash(key);
        let index = hash & (self.capacity - 1);
        let ref mut list = tab[index];

        for i in list.iter_mut() {
            if i.key == key {
                *i = HashEntry {
                    key: key.to_owned(),
                    val: val,
                    hash: hash.clone()
                };
                return;
            }
        }

        list.push(HashEntry {
            key: key.to_owned(),
            val: val,
            hash: hash
        });

        self.count += 1;

    }

    /// get an immutable value from the map if it exists
    pub fn get (&self, key: &[u8]) -> Option<&V> {
        if self.count == 0 {
            None
        } else {
            let tab = match self.table {
                InlineVec::Static(_, ref arr) => (&arr[..]),
                InlineVec::Dynamic(ref vec) => (&vec[..])
            };
            let hash = Self::make_hash(key);
            let index = hash & (self.capacity - 1);
            match tab[index].iter().find(|e| &e.key[..] == key) {
                None => None,
                Some(s) => {
                    let as_raw_ptr = &s.val as *const V;
                    //return some questionably unsafe dereferenced raw pointer
                    Some(unsafe{&*as_raw_ptr})
                }
            }
        }
    }

    /// modifies a {value} located at {key} with {mod_func} if it exists, else inserts a
    /// value into its place wrapped by {put_func}
    pub fn modify_or_else <F1, F2> (&mut self, key: &[u8], mod_func: F1, put_func: F2)
    where F1: Fn(&mut V), F2: FnOnce() -> V {
        let hash = Self::make_hash(key);
        let index = hash & (self.capacity - 1);
        let mut tab = match self.table {
            InlineVec::Static(_, ref mut arr) => &mut arr[..],
            InlineVec::Dynamic(ref mut vec) => &mut vec[..]
        };

        if let Some(s) = tab[index].iter_mut().find(|e| &e.key[..] == key) {
            mod_func(&mut s.val);
            return
        }
        self.count += 1;
        tab[index].push(HashEntry{
            hash: hash,
            key: key.to_owned(),
            val: put_func()
        });
    }

    /// modifies the val for the key.
    /// returns a flag (e.g. to be used if the val is considered empty etc)
    pub fn modify <F1, E> (&mut self, key: &[u8], mod_func: F1) -> Option<E> where F1: Fn(&mut V) -> Option<E> {
        let hash = Self::make_hash(key);
        let index = hash & (self.capacity - 1);
        let mut tab = match self.table {
            InlineVec::Static(_, ref mut arr) => &mut arr[..],
            InlineVec::Dynamic(ref mut vec) => &mut vec[..]
        };
        if let Some(s) = tab[index].iter_mut().find(|e| &e.key[..] == key) {
            mod_func(&mut s.val)
        } else {
            None
        }
    }

    /// removes an entry from the map, returns a bool if successful or not
    pub fn delete (&mut self, key: &[u8]) -> bool {
        if self.count == 0 {
            false
        } else {
            let mut tab = match self.table {
                InlineVec::Static(_, ref mut arr) => &mut arr[..],
                InlineVec::Dynamic(ref mut vec) => &mut vec[..]
            };
            let hash = Self::make_hash(key);
            let index = hash & (self.capacity - 1);
            let del_opt = tab[index].iter().enumerate().find(|&(_, e)| &e.key[..] == key).map(|(i, _)| i);
            match del_opt {
                None => false,
                Some(index_to_delete) => {
                    tab[index].remove(index_to_delete);
                    self.count -= 1;
                    true
                }
            }
        }
    }

    /// applies a function to a value in the hash table, mutably
    pub fn apply <F> (&mut self, key: &[u8], func: F) where F: Fn(&mut V) {
        if self.count == 0 {
            return
        } else {
            let hash = Self::make_hash(key);
            let mut tab = match self.table {
                InlineVec::Static(_, ref mut arr) => (&mut arr[..]),
                InlineVec::Dynamic(ref mut vec) => (&mut vec[..])
            };
            let index = hash & (self.capacity - 1);

            match tab[index].iter_mut().find(|e| &e.key[..] == key) {
                None => (),
                Some(mut s) => {
                    func(&mut s.val);
                }
            };
        }
    }

    /// for now use SipHasher. in the future it may prove too slow. we don't really
    /// need crypto security
    fn make_hash (key: &[u8]) -> usize {
        let mut s = SipHasher::new();
        key.hash(&mut s);
        s.finish() as usize
    }

}

#[derive(Clone)]
pub struct HashEntry <V> {
    key: Vec<u8>,
    val: V,
    hash: usize
}
