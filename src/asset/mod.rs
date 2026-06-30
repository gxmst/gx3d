pub mod gltf_loader;
pub mod mesh;
pub mod procedural;
pub mod texture;

pub use gltf_loader::*;
pub use mesh::*;
pub use procedural::*;
pub use texture::*;

use std::collections::HashMap;
use std::marker::PhantomData;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Handle<T> {
    pub id: u64,
    pub _marker: PhantomData<T>,
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Handle<T> {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }
}

impl<T> Default for Handle<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

pub struct AssetStore<T> {
    assets: HashMap<u64, T>,
    next_id: u64,
}

impl<T> AssetStore<T> {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn insert(&mut self, asset: T) -> Handle<T> {
        let id = self.next_id;
        self.next_id += 1;
        self.assets.insert(id, asset);
        Handle::new(id)
    }

    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.assets.get(&handle.id)
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.assets.get_mut(&handle.id)
    }

    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        self.assets.remove(&handle.id)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn get_all_mut(&mut self) -> impl Iterator<Item = (&u64, &mut T)> {
        self.assets.iter_mut()
    }

    pub fn get_all(&self) -> impl Iterator<Item = (&u64, &T)> {
        self.assets.iter()
    }
}

impl<T> Default for AssetStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AssetManager {
    pub meshes: AssetStore<Mesh>,
    pub textures: AssetStore<Texture>,
    pub materials: AssetStore<Material>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            meshes: AssetStore::new(),
            textures: AssetStore::new(),
            materials: AssetStore::new(),
        }
    }

    pub fn create_default_material(&mut self) -> Handle<Material> {
        let material = Material {
            name: "Default".to_string(),
            albedo_factor: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_factor: [0.0; 3],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
        };
        self.materials.insert(material)
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}
