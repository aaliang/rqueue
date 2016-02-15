use std::hash::{SipHasher, Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::{ptr, mem};

//this is a specialized map.
//keys must be byte slices (they are converted to byte vectors if they are stored)
//values are generic.
pub struct ConcurrentHashMap <V> {
    // for now there are 16 segments
    segments: [Segment<V>; 16]
}

impl <V> ConcurrentHashMap <V> {
    pub fn new () -> ConcurrentHashMap<V> {
        let seg = unsafe {
            let mut seg:[Segment<V>; 16] = mem::uninitialized();
            for i in seg.iter_mut() {
                ptr::write(i, Segment::new());
            }
            seg
        };
        ConcurrentHashMap {
            segments: seg
        }
    }

    /// gets the key from this hashtable if it exists
    pub fn get (&self, key: &[u8]) -> Option<&V> {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].get(key, hash)
    }

    /// applies the function given the value for the key, safely. does no modification
    pub fn get_apply <F> (&self, key: &[u8], func: F) where F: Fn(&mut HashEntry<V>) {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].get_apply(key, hash, func);
    }

    /// Beware! do you really want to use this method?
    /// this is terribly unsafe. returning a mutable reference from here allows bypassing of the rwlock which is extremely dangerous in multithreaded use. use extremely sparingly
    /// if possible use ConcurrentHashMap::get_modify
    pub fn get_mut(&self, key: &[u8]) -> Option<&mut V> {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].get_mut(key, hash)
    }

    /// Searches for key in the hashmap. if a value exists, calls func on the value yielding a new value which is replaced in the same transaction
    pub fn get_modify <F> (&self, key: &[u8], func: F) -> Option<&V> where F: Fn(&V) -> V {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].get_modify(key, hash, func)
    }

    pub fn modify_or_else <F1, F2> (&self, key: &[u8], mod_func: F1, put_func: F2)
    where F1: Fn(&mut V), F2: FnOnce() -> V {
        let hash = Self::make_hash(key);
        let segment = self.segment_index(hash);
        self.segments[segment].modify_or_else(key, hash, mod_func, put_func);
    }

    pub fn insert (&self, key: &[u8], val: V) {
        let hash = Self::make_hash(&key);
        let segment = self.segment_index(hash);
        self.segments[segment].insert(key, hash, val);
    }

    pub fn delete (&self, key: &[u8]) -> bool {
        let hash = Self::make_hash(&key);
        let segment = self.segment_index(hash);
        self.segments[segment].delete(&key, hash)
    }

    /// gets the segment number given a hash
    fn segment_index (&self, hash: usize) -> usize {
        hash >> 4 & (self.segments.len() - 1)
    }

    fn make_hash (key: &[u8]) -> usize {
        let mut s = SipHasher::new();
        key.hash(&mut s);
        s.finish() as usize
    }
}

/// this is silly and awful. for small enough lists < 64 items we just use a stack allocated array
/// kind of pointless given that the LinkedLists are heap allocated anyways and we use them for each bucket.
enum InlineVec <T> {
    Static(usize, [Vec<T>; 64]),
    Dynamic(Vec<Vec<T>>)
}

/// A Segment is a write locked subset of a hashmap. Multiple reads can be done concurrently on a single segment.
pub struct Segment <V> {
    count: AtomicUsize,
    table: RwLock<InlineVec<HashEntry<V>>>,
    capacity: usize
}

impl <V> Segment <V> {
    pub fn new () -> Segment <V> {
        let stat = unsafe {
            let mut stat:[Vec<HashEntry<V>>; 64] = mem::uninitialized();
            for i in stat.iter_mut() {
                ptr::write(i, Vec::new());
            }
            stat
        };
        Segment {
            count: AtomicUsize::new(0),
            table: RwLock::new(InlineVec::Static(0, stat)),
            capacity: 8
        }
    }
    pub fn insert (&self, key: &[u8], hash: usize, val: V) {
        let mut table = self.table.write().unwrap();
        let mut tab = match *table {
            InlineVec::Static(_, ref mut arr) => &mut arr[..],
            InlineVec::Dynamic(ref mut vec) => &mut vec[..]
        };

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

        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get (&self, key: &[u8], hash: usize) -> Option<&V> {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            None
        } else {
            let read = self.table.read().unwrap();
            let tab = match *read {
                InlineVec::Static(_, ref arr) => (&arr[..]),
                InlineVec::Dynamic(ref vec) => (&vec[..])
            };
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

    pub fn get_apply <F> (&self, key: &[u8], hash: usize, func: F) where F: Fn(&mut HashEntry<V>) {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            return
        } else {
            let mut read = self.table.write().unwrap();
            let mut tab = match *read {
                InlineVec::Static(_, ref mut arr) => (&mut arr[..]),
                InlineVec::Dynamic(ref mut vec) => (&mut vec[..])
            };
            let index = hash & (self.capacity - 1);

            //let ref mut x = tab[index][0];
            match tab[index].iter_mut().find(|e| &e.key[..] == key) {
                None => (),
                Some(mut s) => {
                    func(&mut s);
                }
            };
        }
    }

    //see ConcurrentHashMap::get_mut above. you probably don't want to use this
    pub fn get_mut (&self, key: &[u8], hash: usize) -> Option<&mut V> {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            None
        } else {
            let mut write = self.table.write().unwrap();
            let mut tab = match *write {
                InlineVec::Static(_, ref mut arr) => &mut arr[..],
                InlineVec::Dynamic(ref mut vec) => &mut vec[..]
            };
            let index = hash & (self.capacity - 1);
            match tab[index].iter_mut().find(|e| &e.key[..] == key) {
                None => None,
                Some(s) => {
                    let as_raw_ptr = &mut s.val as *mut V;
                    Some(unsafe{&mut *as_raw_ptr})
                }
            }
        }
    }

    pub fn get_modify <F> (&self, key: &[u8], hash: usize, func: F) -> Option<&V> where F: Fn(&V) -> V {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            None
        } else {
            let mut write = self.table.write().unwrap();
            let mut tab = match *write {
                InlineVec::Static(_, ref mut arr) => &mut arr[..],
                InlineVec::Dynamic(ref mut vec) => &mut vec[..]
            };
            let index = hash & (self.capacity - 1);
            match tab[index].iter_mut().find(|e| &e.key[..] == key) {
                None => None,
                Some(s) =>  {
                    let new_val = func(&s.val);
                    *s = HashEntry {
                        hash: hash,
                        key: key.to_owned(),
                        val: new_val
                    };
                    let as_raw_ptr = &mut s.val as *mut V;
                    Some(unsafe{&mut *as_raw_ptr})
                }
            }
        }
    }

    pub fn modify_or_else <F1, F2> (&self, key: &[u8], hash: usize, mod_func: F1, put_func: F2) where F1: Fn(&mut V), F2: FnOnce() -> V {
        let mut write = self.table.write().unwrap();
        let mut tab = match *write {
            InlineVec::Static(_, ref mut arr) => &mut arr[..],
            InlineVec::Dynamic(ref mut vec) => &mut vec[..]
        };
        let index = hash & (self.capacity - 1);
        if let Some(s) = tab[index].iter_mut().find(|e| &e.key[..] == key) {
            mod_func(&mut s.val);
            return
        } 
        //else
        self.count.fetch_add(1, Ordering::SeqCst);
        tab[index].push(HashEntry{
            hash: hash,
            key: key.to_owned(),
            val: put_func()
        });
    }


    pub fn delete (&self, key: &[u8], hash: usize) -> bool {
        let count = self.count.load(Ordering::SeqCst);
        if count == 0 {
            false
        } else {
            let mut write = self.table.write().unwrap();
            let mut tab = match *write {
                InlineVec::Static(_, ref mut arr) => &mut arr[..],
                InlineVec::Dynamic(ref mut vec) => &mut vec[..]
            };
            let index = hash & (self.capacity - 1);
            let del_opt = tab[index].iter().enumerate().find(|&(_, e)| &e.key[..] == key).map(|(i, _)| i);
            match del_opt {
                None => false,
                Some(index_to_delete) => {
                    tab[index].remove(index_to_delete);
                    self.count.fetch_sub(1, Ordering::SeqCst);
                    true
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct HashEntry <V> {
    key: Vec<u8>,
    val: V,
    hash: usize
}
