use std::any::{type_name, Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

type ResourceCell = Rc<RefCell<Box<dyn Any>>>;

/// Type-map of engine singletons (renderer, physics world, input state, ...).
///
/// Every resource lives in its own `RefCell`, so borrowing one resource never
/// conflicts with borrowing another: a system may hold `InputState` immutably
/// while mutating `Weapon`, and may insert new resources while others are
/// borrowed. Only two borrows of the *same* resource conflict, and that
/// conflict panics with the resource's type name (it is always a bug).
pub struct Resources {
    storage: RefCell<HashMap<TypeId, ResourceCell>>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            storage: RefCell::new(HashMap::new()),
        }
    }

    /// Insert or replace a resource. Replacing a resource that is currently
    /// borrowed is allowed: existing guards keep the old value alive, while
    /// subsequent lookups observe the new one.
    pub fn insert<T: 'static>(&self, value: T) {
        self.storage
            .borrow_mut()
            .insert(TypeId::of::<T>(), Rc::new(RefCell::new(Box::new(value))));
    }

    /// Shared borrow. `None` means the resource is missing; a live exclusive
    /// borrow of the same resource panics instead of being masked as `None`.
    #[track_caller]
    pub fn get<T: 'static>(&self) -> Option<Res<T>> {
        let cell = self.storage.borrow().get(&TypeId::of::<T>()).cloned()?;
        Some(Res::new(cell))
    }

    /// Exclusive borrow. `None` means the resource is missing; any live borrow
    /// of the same resource panics instead of being masked as `None`.
    #[track_caller]
    pub fn get_mut<T: 'static>(&self) -> Option<ResMut<T>> {
        let cell = self.storage.borrow().get(&TypeId::of::<T>()).cloned()?;
        Some(ResMut::new(cell))
    }

    /// Like [`get`](Self::get), but panics with a descriptive message naming
    /// the resource type and caller location when the resource is missing.
    #[track_caller]
    pub fn expect<T: 'static>(&self) -> Res<T> {
        self.get::<T>().unwrap_or_else(|| missing::<T, _>())
    }

    /// Like [`get_mut`](Self::get_mut), but panics with a descriptive message
    /// naming the resource type and caller location when the resource is missing.
    #[track_caller]
    pub fn expect_mut<T: 'static>(&self) -> ResMut<T> {
        self.get_mut::<T>().unwrap_or_else(|| missing::<T, _>())
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.storage.borrow().contains_key(&TypeId::of::<T>())
    }

    /// Take a resource out of the container by value.
    ///
    /// Panics if the resource is currently borrowed — a removed-while-borrowed
    /// resource has no owner to return to, and the old container silently hit
    /// the same situation as an opaque `RefCell` panic.
    #[track_caller]
    pub fn remove<T: 'static>(&self) -> Option<T> {
        let cell = self.storage.borrow_mut().remove(&TypeId::of::<T>())?;
        match Rc::try_unwrap(cell) {
            Ok(refcell) => refcell.into_inner().downcast::<T>().ok().map(|b| *b),
            Err(_) => panic!(
                "Resource `{}` cannot be removed while it is borrowed",
                type_name::<T>()
            ),
        }
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

#[track_caller]
fn missing<T: 'static, R>() -> R {
    panic!(
        "Resource `{}` is missing (was it inserted during setup?)",
        type_name::<T>()
    )
}

/// Shared borrow of a resource.
///
/// The guard owns a strong reference to the resource's cell, so it remains
/// valid even if the resource is replaced in (or removed from) the container
/// while this borrow is alive.
pub struct Res<T: 'static> {
    // Field order is load-bearing: `guard` must drop before `cell` so the
    // borrow flag is released while the RefCell is still alive.
    guard: Ref<'static, T>,
    _cell: ResourceCell,
}

impl<T: 'static> Res<T> {
    #[track_caller]
    fn new(cell: ResourceCell) -> Self {
        let borrow = match cell.try_borrow() {
            Ok(borrow) => borrow,
            Err(_) => panic!(
                "Resource `{}` is already mutably borrowed",
                type_name::<T>()
            ),
        };
        let guard = Ref::map(borrow, |boxed| {
            boxed
                .downcast_ref::<T>()
                .expect("resource cell type matches its TypeId key")
        });
        // SAFETY: `guard` points into the RefCell owned by `cell` (an Rc heap
        // allocation, so it never moves). `_cell` keeps that allocation alive
        // for as long as this struct exists, and the field order above drops
        // the borrow before the cell. Extending the lifetime to 'static is
        // therefore sound because the referent outlives the guard.
        let guard: Ref<'static, T> = unsafe { std::mem::transmute(guard) };
        Self { guard, _cell: cell }
    }
}

impl<T: 'static> Deref for Res<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard
    }
}

/// Exclusive borrow of a resource. See [`Res`] for the aliasing guarantees.
pub struct ResMut<T: 'static> {
    // Same drop-order invariant as `Res`.
    guard: RefMut<'static, T>,
    _cell: ResourceCell,
}

impl<T: 'static> ResMut<T> {
    #[track_caller]
    fn new(cell: ResourceCell) -> Self {
        let borrow = match cell.try_borrow_mut() {
            Ok(borrow) => borrow,
            Err(_) => panic!("Resource `{}` is already borrowed", type_name::<T>()),
        };
        let guard = RefMut::map(borrow, |boxed| {
            boxed
                .downcast_mut::<T>()
                .expect("resource cell type matches its TypeId key")
        });
        // SAFETY: identical reasoning to `Res::new`.
        let guard: RefMut<'static, T> = unsafe { std::mem::transmute(guard) };
        Self { guard, _cell: cell }
    }
}

impl<T: 'static> Deref for ResMut<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T: 'static> DerefMut for ResMut<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

#[cfg(test)]
mod tests {
    use super::Resources;

    #[test]
    fn different_resources_can_be_borrowed_mutably_at_the_same_time() {
        let resources = Resources::new();
        resources.insert(1_u32);
        resources.insert(String::from("hello"));

        let number = resources.get::<u32>().unwrap();
        let mut text = resources.get_mut::<String>().unwrap();
        text.push_str(" world");
        assert_eq!(*number, 1);
        assert_eq!(&*text, "hello world");
    }

    #[test]
    fn inserting_while_another_resource_is_borrowed_is_allowed() {
        let resources = Resources::new();
        resources.insert(1_u32);

        let number = resources.get::<u32>().unwrap();
        resources.insert(String::from("late"));
        assert_eq!(*number, 1);
        assert_eq!(&*resources.expect::<String>(), "late");
    }

    #[test]
    fn replacing_a_borrowed_resource_keeps_existing_guards_valid() {
        let resources = Resources::new();
        resources.insert(String::from("old"));

        let old = resources.get::<String>().unwrap();
        resources.insert(String::from("new"));
        assert_eq!(&*old, "old");
        assert_eq!(&*resources.expect::<String>(), "new");
    }

    #[test]
    #[should_panic(expected = "already mutably borrowed")]
    fn conflicting_borrows_of_the_same_resource_panic_with_the_type_name() {
        let resources = Resources::new();
        resources.insert(1_u32);

        let _held = resources.get_mut::<u32>().unwrap();
        let _conflict = resources.get::<u32>();
    }

    #[test]
    fn remove_returns_the_value_and_missing_lookups_return_none() {
        let resources = Resources::new();
        resources.insert(7_u32);

        assert_eq!(resources.remove::<u32>(), Some(7));
        assert!(resources.get::<u32>().is_none());
        assert!(resources.remove::<u32>().is_none());
    }
}
