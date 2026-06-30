use crate::input::InputState;
use glam::Vec2;
use winit::event::MouseButton;

pub struct Weapon {
    pub name: String,
    pub damage: f32,
    pub fire_rate: f32,
    pub recoil_pattern: Vec2,
    pub current_ammo: u32,
    pub max_ammo: u32,
    pub is_reloading: bool,
    pub reload_duration: f32,
    pub reload_timer: f32,
    pub fire_cooldown: f32,
}

impl Weapon {
    pub fn new(name: &str, damage: f32, fire_rate: f32, max_ammo: u32) -> Self {
        Self {
            name: name.to_string(),
            damage,
            fire_rate,
            recoil_pattern: Vec2::new(0.02, 0.05),
            current_ammo: max_ammo,
            max_ammo,
            is_reloading: false,
            reload_duration: 2.0,
            reload_timer: 0.0,
            fire_cooldown: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32, input: &InputState) -> bool {
        if self.fire_cooldown > 0.0 {
            self.fire_cooldown -= dt;
        }

        if self.is_reloading {
            self.reload_timer -= dt;
            if self.reload_timer <= 0.0 {
                self.current_ammo = self.max_ammo;
                self.is_reloading = false;
            }
            return false;
        }

        if input.is_mouse_pressed(MouseButton::Left)
            && self.fire_cooldown <= 0.0
            && self.current_ammo > 0
        {
            self.current_ammo -= 1;
            self.fire_cooldown = 1.0 / self.fire_rate;
            return true;
        }

        if self.current_ammo == 0 {
            self.start_reload();
        }

        false
    }

    pub fn start_reload(&mut self) {
        if !self.is_reloading && self.current_ammo < self.max_ammo {
            self.is_reloading = true;
            self.reload_timer = self.reload_duration;
        }
    }

    pub fn get_recoil(&self) -> Vec2 {
        self.recoil_pattern
    }
}

pub fn create_default_weapon() -> Weapon {
    Weapon::new("Rifle", 25.0, 10.0, 30)
}
