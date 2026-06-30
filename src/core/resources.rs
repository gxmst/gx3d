use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;

pub struct Resources {
    storage: RefCell<HashMap<TypeId, Box<dyn Any>>>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            storage: RefCell::new(HashMap::new()),
        }
    }

    pub fn insert<T: 'static>(&self, value: T) {
        self.storage
            .borrow_mut()
            .insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get<T: 'static>(&self) -> Option<Ref<'_, T>> {
        let borrow = self.storage.borrow();
        // Ensure the requested type exists before creating the map Ref.
        if !borrow.contains_key(&TypeId::of::<T>()) {
            return None;
        }
        Some(Ref::map(borrow, |map| {
            map.get(&TypeId::of::<T>())
                .unwrap()
                .downcast_ref::<T>()
                .unwrap()
        }))
    }

    pub fn get_mut<T: 'static>(&self) -> Option<RefMut<'_, T>> {
        let borrow = self.storage.borrow_mut();
        // Ensure the requested type exists before creating the map RefMut.
        if !borrow.contains_key(&TypeId::of::<T>()) {
            return None;
        }
        Some(RefMut::map(borrow, |map| {
            map.get_mut(&TypeId::of::<T>())
                .unwrap()
                .downcast_mut::<T>()
                .unwrap()
        }))
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.storage.borrow().contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: 'static>(&self) -> Option<T> {
        self.storage
            .borrow_mut()
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast().ok())
            .map(|b| *b)
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}
