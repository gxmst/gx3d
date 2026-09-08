//! Buy menu and match economy.
//!
//! In match mode, pressing B during warmup (or anytime in non-match scenes)
//! opens a lightweight overlay listing the weapon catalog; digit keys 1-5
//! purchase. Money is earned per kill and per round result. Outside match
//! mode everything is free, so the menu doubles as a weapon selector for the
//! sandbox scenes.

use crate::core::{EngineWorld, Resources};
use crate::game::weapon::{Weapon, DEFAULT_WEAPON_INDEX, WEAPON_CATALOG};
use crate::game::WeaponModel;
use crate::input::InputState;
use winit::keyboard::KeyCode;

use super::Toast;

pub const KILL_REWARD: u32 = 300;
pub const ROUND_WIN_REWARD: u32 = 1400;
pub const ROUND_LOSS_REWARD: u32 = 900;
pub const STARTING_MONEY: u32 = 800;
pub const MAX_MONEY: u32 = 16000;

/// Player wallet + buy menu open state. Only meaningful in match mode; in
/// sandbox scenes `money` is ignored (prices treated as 0).
pub struct BuyState {
    pub open: bool,
    pub money: u32,
    /// Purchases are free outside match mode.
    pub economy_enabled: bool,
}

impl BuyState {
    pub fn new(economy_enabled: bool) -> Self {
        Self {
            open: false,
            money: STARTING_MONEY,
            economy_enabled,
        }
    }

    pub fn award(&mut self, amount: u32) {
        self.money = (self.money + amount).min(MAX_MONEY);
    }
}

pub fn update(_world: &mut EngineWorld, resources: &Resources) {
    if super::menu_open(resources) {
        // The pause menu covers everything; drop the buy overlay so it does
        // not linger underneath (and stale-capture digits on resume).
        if let Some(mut buy) = resources.get_mut::<BuyState>() {
            buy.open = false;
        }
        return;
    }
    let spectating = resources
        .get::<super::match_mode::MatchState>()
        .map(|state| state.player_spectating)
        .unwrap_or(false);
    let Some(mut buy) = resources.get_mut::<BuyState>() else {
        return;
    };
    if spectating {
        buy.open = false;
        return;
    }

    let (toggle, digits) = {
        let input = resources.expect::<InputState>();
        let digits = [
            input.is_key_just_pressed(KeyCode::Digit1),
            input.is_key_just_pressed(KeyCode::Digit2),
            input.is_key_just_pressed(KeyCode::Digit3),
            input.is_key_just_pressed(KeyCode::Digit4),
            input.is_key_just_pressed(KeyCode::Digit5),
        ];
        (input.is_key_just_pressed(KeyCode::KeyB), digits)
    };

    // B toggles the menu. In match mode, buying is restricted to warmup so
    // mid-fight upgrades stay impossible (mirrors buy-time rules).
    if toggle {
        let allowed = resources
            .get::<super::match_mode::MatchState>()
            .map(|state| {
                matches!(
                    state.phase,
                    super::match_mode::RoundPhase::Warmup
                        | super::match_mode::RoundPhase::RoundOver { .. }
                )
            })
            .unwrap_or(true);
        if buy.open {
            buy.open = false;
        } else if allowed {
            buy.open = true;
        } else if let Some(mut toast) = resources.get_mut::<Toast>() {
            toast.show("只能在回合准备阶段购买", 1.5);
        }
    }
    if !buy.open {
        return;
    }

    let Some(choice) = digits.iter().position(|pressed| *pressed) else {
        return;
    };
    let spec = WEAPON_CATALOG[choice];
    let price = if buy.economy_enabled { spec.price } else { 0 };
    if buy.money < price && buy.economy_enabled {
        if let Some(mut toast) = resources.get_mut::<Toast>() {
            toast.show(format!("资金不足：{} 需要 ${}", spec.name, spec.price), 2.0);
        }
        return;
    }
    if buy.economy_enabled {
        buy.money -= price;
    }
    buy.open = false;
    drop(buy);

    equip_weapon(resources, choice);
    if let Some(mut toast) = resources.get_mut::<Toast>() {
        toast.show(format!("已购买 {}", spec.name), 1.5);
    }
}

/// Swap the player's weapon to catalog entry `index` and play the draw
/// animation. Ammo refills; the old weapon is discarded (no inventory).
pub fn equip_weapon(resources: &Resources, index: usize) {
    resources.insert(Weapon::from_spec(index));
    if let Some(mut model) = resources.get_mut::<WeaponModel>() {
        // Swap in the matching first-person mesh so each gun reads distinct.
        if let Some(meshes) = resources.get::<super::WeaponMeshes>() {
            if let Some(mesh) = meshes.0.get(index).copied() {
                model.mesh = mesh;
            }
        }
        model.start_draw_anim();
    }
}

/// Round-start reset used by match mode: back to the default pistol.
pub fn reset_to_default_weapon(resources: &Resources) {
    equip_weapon(resources, DEFAULT_WEAPON_INDEX);
}

/// Lines shown by the buy overlay.
pub fn menu_lines(buy: &BuyState, current_index: usize) -> Vec<String> {
    WEAPON_CATALOG
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let marker = if i == current_index { "●" } else { " " };
            let price = if buy.economy_enabled {
                format!("${}", spec.price)
            } else {
                "免费".to_string()
            };
            format!(
                "{marker} {}  {}   {}   伤害{:.0}  射速{:.1}  弹匣{}",
                i + 1,
                spec.name,
                price,
                spec.damage,
                spec.fire_rate,
                spec.magazine
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn award_saturates_at_cap() {
        let mut buy = BuyState::new(true);
        buy.money = MAX_MONEY - 100;
        buy.award(500);
        assert_eq!(buy.money, MAX_MONEY);
    }

    #[test]
    fn catalog_covers_five_archetypes_with_free_default() {
        assert_eq!(WEAPON_CATALOG.len(), 5);
        assert_eq!(WEAPON_CATALOG[DEFAULT_WEAPON_INDEX].price, 0);
        assert!(WEAPON_CATALOG.iter().any(|spec| spec.pellets > 1));
    }

    #[test]
    fn menu_lines_mark_current_weapon() {
        let buy = BuyState::new(true);
        let lines = menu_lines(&buy, 2);
        assert!(lines[2].starts_with('●'));
        assert!(!lines[0].starts_with('●'));
    }
}
