use super::Handle;

#[derive(Debug)]
pub struct Material {
    pub name: String,
    pub albedo_factor: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_factor: [f32; 3],
    pub albedo_map: Option<Handle<Texture>>,
    pub normal_map: Option<Handle<Texture>>,
    pub metallic_roughness_map: Option<Handle<Texture>>,
    pub emissive_map: Option<Handle<Texture>>,
    /// Enables the dedicated water shading path (fresnel, animated micro
    /// ripples, whitecap foam) and implies transparency.
    pub water: bool,
}

impl Material {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            albedo_factor: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_factor: [0.0; 3],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
            water: false,
        }
    }

    pub fn white() -> Self {
        Self {
            name: "White".to_string(),
            albedo_factor: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_factor: [0.0; 3],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
            water: false,
        }
    }

    pub fn gray() -> Self {
        Self {
            name: "Gray".to_string(),
            albedo_factor: [0.5, 0.5, 0.5, 1.0],
            metallic: 0.0,
            roughness: 0.7,
            emissive_factor: [0.0; 3],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
            water: false,
        }
    }

    pub fn metal(color: [f32; 3]) -> Self {
        Self {
            name: "Metal".to_string(),
            albedo_factor: [color[0], color[1], color[2], 1.0],
            metallic: 1.0,
            roughness: 0.2,
            emissive_factor: [0.0; 3],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
            water: false,
        }
    }
}

#[derive(Debug)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub format: TextureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8,
}

impl Texture {
    pub fn new(name: &str, width: u32, height: u32, data: Vec<u8>, format: TextureFormat) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            data,
            format,
        }
    }

    pub fn from_rgba8(name: &str, width: u32, height: u32, data: Vec<u8>) -> Self {
        Self::new(name, width, height, data, TextureFormat::Rgba8)
    }
}
