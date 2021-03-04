use std::any::Any;
use std::collections::btree_map;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::dispatch::DispCtx;

pub struct Inventory {
    inner: Mutex<InventoryInner>,
}
impl Inventory {
    pub(crate) fn new() -> Self {
        Self { inner: Mutex::new(InventoryInner::default()) }
    }

    pub fn register<T: Entity>(&self, ent: Arc<T>, name: String) -> EntityID {
        let any = Arc::clone(&ent) as Arc<dyn Any + Send + Sync>;
        let entptr = &*ent as *const T as usize;
        let rec = Record { any, ent, name };

        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id();
        inner.entities.insert(id, rec);
        inner.reverse.insert(entptr, id);

        id
    }
    pub fn print(&self) {
        let inner = self.inner.lock().unwrap();
        for (id, rec) in inner.entities.iter() {
            println!(
                "{:x}: {} {:x?}",
                id.num, rec.name, &*rec.any as *const dyn Any
            );
        }
    }
    pub fn iter_over(&self, f: impl Fn(&mut Iter)) {
        let inner = self.inner.lock().unwrap();
        let mut iter = Iter { inner: inner.entities.iter() };

        f(&mut iter);
    }
}

pub struct Iter<'a> {
    inner: btree_map::Iter<'a, EntityID, Record>,
}
impl<'a> std::iter::Iterator for Iter<'a> {
    type Item = &'a Arc<dyn Entity>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some((_id, rec)) => Some(&rec.ent),
            None => None,
        }
    }
}

#[derive(Default)]
struct InventoryInner {
    entities: BTreeMap<EntityID, Record>,
    reverse: HashMap<usize, EntityID>,
    next_id: usize,
}
impl InventoryInner {
    fn next_id(&mut self) -> EntityID {
        self.next_id += 1;
        EntityID { num: self.next_id }
    }
}

// XXX: still a WIP
#[allow(unused)]
struct Record {
    any: Arc<dyn Any + Send + Sync + 'static>,
    ent: Arc<dyn Entity>,
    name: String,
}

pub trait Entity: Send + Sync + 'static {
    #[allow(unused_variables)]
    fn quiesce(&self, ctx: &DispCtx) {}
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntityID {
    num: usize,
}
