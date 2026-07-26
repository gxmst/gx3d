use crate::input::InputState;
use glam::Vec2;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Static description of one purchasable weapon. All stats are original
/// archetypes (pistol / SMG / rifle / marksman / shotgun), not tied to any
/// specific real or licensed firearm.
#[derive(Debug, Clone, Copy)]
pub struct WeaponSpec {
    pub name: &'static str,
    pub damage: f32,
    pub fire_rate: f32,
    pub magazine: u32,
    pub reload_duration: f32,
    pub recoil: Vec2,
    pub price: u32,
    /// Pellets per trigger pull (>1 = shotgun spread, applied as repeated
    /// damage rolls on one raycast target).
    pub pellets: u32,
}

/// The buyable arsenal, indexed by the buy menu's 1..=5 keys.
pub const WEAPON_CATALOG: [WeaponSpec; 5] = [
    WeaponSpec {
        name: "GX-9 手枪",
        damage: 18.0,
        fire_rate: 4.5,
        magazine: 12,
        reload_duration: 1.3,
        recoil: Vec2::new(0.012, 0.03),
        price: 0,
        pellets: 1,
    },
    WeaponSpec {
        name: "GX-45 冲锋枪",
        damage: 14.0,
        fire_rate: 13.0,
        magazine: 32,
        reload_duration: 1.9,
        recoil: Vec2::new(0.02, 0.038),
        price: 1200,
        pellets: 1,
    },
    WeaponSpec {
        name: "GX-7 步枪",
        damage: 25.0,
        fire_rate: 10.0,
        magazine: 30,
        reload_duration: 2.0,
        recoil: Vec2::new(0.02, 0.05),
        price: 2700,
        pellets: 1,
    },
    WeaponSpec {
        name: "GX-50 射手步枪",
        damage: 70.0,
        fire_rate: 1.1,
        magazine: 8,
        reload_duration: 2.6,
        recoil: Vec2::new(0.03, 0.10),
        price: 4200,
        pellets: 1,
    },
    WeaponSpec {
        name: "GX-12 霰弹枪",
        damage: 9.0,
        fire_rate: 1.4,
        magazine: 7,
        reload_duration: 2.8,
        recoil: Vec2::new(0.035, 0.09),
        price: 1800,
        pellets: 6,
    },
];

/// Index of the free fallback weapon (round-start default in match mode).
pub const DEFAULT_WEAPON_INDEX: usize = 0;
/// Index of the rifle used by non-match scenes.
pub const SANDBOX_WEAPON_INDEX: usize = 2;

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
    pub pellets: u32,
    /// Index into [`WEAPON_CATALOG`] this instance was built from.
    pub catalog_index: usize,
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
            pellets: 1,
            catalog_index: SANDBOX_WEAPON_INDEX,
        }
    }

    pub fn from_spec(index: usize) -> Self {
        let spec = WEAPON_CATALOG
            .get(index)
            .copied()
            .unwrap_or(WEAPON_CATALOG[DEFAULT_WEAPON_INDEX]);
        Self {
            name: spec.name.to_string(),
            damage: spec.damage,
            fire_rate: spec.fire_rate,
            recoil_pattern: spec.recoil,
            current_ammo: spec.magazine,
            max_ammo: spec.magazine,
            is_reloading: false,
            reload_duration: spec.reload_duration,
            reload_timer: 0.0,
            fire_cooldown: 0.0,
            pellets: spec.pellets.max(1),
            catalog_index: index.min(WEAPON_CATALOG.len() - 1),
        }
    }

    /// Total damage of one trigger pull (all pellets on target).
    pub fn volley_damage(&self) -> f32 {
        self.damage * self.pellets as f32
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

        if input.is_key_just_pressed(KeyCode::KeyR) {
            self.start_reload();
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

#[cfg(test)]
mod tests {
    use super::Weapon;
    use crate::input::InputState;
    use winit::{event::ElementState, keyboard::KeyCode};

    #[test]
    fn reload_key_starts_reload_when_magazine_is_not_full() {
        let mut weapon = Weapon::new("Test", 10.0, 5.0, 30);
        weapon.current_ammo = 7;
        let mut input = InputState::default();
        input.process_key(KeyCode::KeyR, ElementState::Pressed);
        assert!(!weapon.update(1.0 / 60.0, &input));
        assert!(weapon.is_reloading);
    }
}
