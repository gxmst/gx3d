//! Weather: clear / rain / snow, selectable in the pause menu or randomized.
//!
//! Precipitation is a pool of thin streak (rain) or small flake (snow)
//! entities recycled around the camera — no spawning or despawning during a
//! storm, only teleporting particles that fall below ground back to the top
//! of the volume. Weather also dims the directional light contribution via
//! `SceneLights` scaling and drives an ambient rain loop.

use crate::core::{EngineWorld, Resources, Time, Transform};
use crate::renderer::{Camera, RenderMesh};
use glam::{Quat, Vec3};

/// Particle volume around the camera. Particles wrap within this box, so the
/// storm follows the player without ever running out of drops.
const VOLUME_HALF_XZ: f32 = 26.0;
const VOLUME_TOP: f32 = 22.0;
const VOLUME_BOTTOM: f32 = -2.0;
const RAIN_COUNT: usize = 420;
const SNOW_COUNT: usize = 300;
const RAIN_FALL_SPEED: f32 = 19.0;
const SNOW_FALL_SPEED: f32 = 2.1;
/// Seconds between automatic switches in Random mode.
const RANDOM_MIN_HOLD: f32 = 60.0;
const RANDOM_SPAN: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum WeatherKind {
    #[default]
    Clear,
    Rain,
    Snow,
    /// Cycles between the three concrete kinds on a timer.
    Random,
}

impl WeatherKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Clear => "晴天",
            Self::Rain => "雨天",
            Self::Snow => "雪天",
            Self::Random => "随机",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Clear => Self::Rain,
            Self::Rain => Self::Snow,
            Self::Snow => Self::Random,
            Self::Random => Self::Clear,
        }
    }
}

/// Marks one precipitation particle. `seed` decorrelates drift phases.
#[derive(Debug, Clone, Copy)]
pub struct WeatherParticle {
    pub seed: f32,
}

/// Engine-side weather state (resource). The *selected* kind is what the user
/// picked (possibly Random); the *active* kind is what is falling right now.
pub struct WeatherState {
    pub selected: WeatherKind,
    pub active: WeatherKind,
    /// Countdown to the next automatic switch in Random mode.
    random_timer: f32,
    /// Deterministic RNG state for random picks.
    rng: u32,
    /// Live particle entities (rain or snow, never both).
    particles: Vec<hecs::Entity>,
    rain_material: crate::asset::Handle<crate::asset::Material>,
    snow_material: crate::asset::Handle<crate::asset::Material>,
    particle_mesh: crate::asset::Handle<crate::asset::Mesh>,
    /// Handle of the looping rain ambience, if currently playing.
    rain_sound_playing: bool,
}

impl WeatherState {
    pub fn new(
        rain_material: crate::asset::Handle<crate::asset::Material>,
        snow_material: crate::asset::Handle<crate::asset::Material>,
        particle_mesh: crate::asset::Handle<crate::asset::Mesh>,
        selected: WeatherKind,
    ) -> Self {
        Self {
            selected,
            active: WeatherKind::Clear,
            random_timer: 0.0,
            rng: 0x9E37_79B9,
            particles: Vec::new(),
            rain_material,
            snow_material,
            particle_mesh,
            rain_sound_playing: false,
        }
    }

    fn next_random(&mut self) -> WeatherKind {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        match (self.rng >> 16) % 3 {
            0 => WeatherKind::Clear,
            1 => WeatherKind::Rain,
            _ => WeatherKind::Snow,
        }
    }

    fn rand01(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.rng >> 8) as f32 / (u32::MAX >> 8) as f32
    }
}

/// Update: resolve the target kind (following Random switches), rebuild the
/// particle pool on changes, and advance every particle.
pub fn update(world: &mut EngineWorld, resources: &Resources) {
    let Some(mut weather) = resources.get_mut::<WeatherState>() else {
        return;
    };
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds())
        .unwrap_or(0.0);
    let camera_position = resources
        .get::<Camera>()
        .map(|c| c.position)
        .unwrap_or(Vec3::ZERO);

    // Resolve the concrete kind that should be falling.
    let target = match weather.selected {
        WeatherKind::Random => {
            weather.random_timer -= dt;
            if weather.random_timer <= 0.0 {
                let pick = weather.next_random();
                let hold = RANDOM_MIN_HOLD + weather.rand01() * RANDOM_SPAN;
                weather.random_timer = hold;
                pick
            } else {
                weather.active
            }
        }
        concrete => concrete,
    };

    if target != weather.active {
        switch_weather(world, resources, &mut weather, target, camera_position);
    }

    advance_particles(world, &mut weather, dt, camera_position);
}

fn switch_weather(
    world: &mut EngineWorld,
    resources: &Resources,
    weather: &mut WeatherState,
    target: WeatherKind,
    camera_position: Vec3,
) {
    // Tear down the old pool.
    for entity in weather.particles.drain(..) {
        world.despawn(entity);
    }
    weather.active = target;

    // Ambient rain loop follows the active kind.
    let want_rain_sound = target == WeatherKind::Rain;
    if want_rain_sound != weather.rain_sound_playing {
        if let Some(mut audio) = resources.get_mut::<Option<crate::audio::AudioSystem>>() {
            if let Some(ref mut audio) = *audio {
                if want_rain_sound {
                    audio.start_rain_loop();
                } else {
                    audio.stop_rain_loop();
                }
            }
        }
        weather.rain_sound_playing = want_rain_sound;
    }

    let (count, material, scale) = match target {
        WeatherKind::Clear | WeatherKind::Random => return,
        WeatherKind::Rain => (
            RAIN_COUNT,
            weather.rain_material,
            // Long thin streak: rain reads through motion-stretched shape.
            Vec3::new(0.015, 0.55, 0.015),
        ),
        WeatherKind::Snow => (SNOW_COUNT, weather.snow_material, Vec3::splat(0.05)),
    };

    for i in 0..count {
        let entity = world.spawn();
        let fx = weather.rand01();
        let fz = weather.rand01();
        let fy = weather.rand01();
        let seed = weather.rand01();
        let position = camera_position
            + Vec3::new(
                (fx - 0.5) * VOLUME_HALF_XZ * 2.0,
                VOLUME_BOTTOM + fy * (VOLUME_TOP - VOLUME_BOTTOM),
                (fz - 0.5) * VOLUME_HALF_XZ * 2.0,
            );
        world.add_component(entity, Transform::new(position, Quat::IDENTITY, scale));
        world.add_component(
            entity,
            RenderMesh {
                mesh: weather.particle_mesh,
                material,
            },
        );
        world.add_component(entity, WeatherParticle { seed });
        weather.particles.push(entity);
        let _ = i;
    }
    log::info!("Weather switched to {:?} ({count} particles)", target);
}

fn advance_particles(
    world: &mut EngineWorld,
    weather: &mut WeatherState,
    dt: f32,
    camera_position: Vec3,
) {
    if weather.particles.is_empty() {
        return;
    }
    let (fall_speed, drift) = match weather.active {
        WeatherKind::Rain => (RAIN_FALL_SPEED, 0.6),
        WeatherKind::Snow => (SNOW_FALL_SPEED, 1.4),
        _ => return,
    };
    let floor = camera_position.y + VOLUME_BOTTOM;
    let ceiling = camera_position.y + VOLUME_TOP;
    for (transform, particle) in world
        .ecs
        .query::<(&mut Transform, &WeatherParticle)>()
        .iter()
    {
        let phase = particle.seed * std::f32::consts::TAU;
        transform.position.y -= fall_speed * (0.8 + particle.seed * 0.4) * dt;
        // Sideways drift: snow meanders, rain leans slightly.
        transform.position.x += (phase + transform.position.y * 0.35).sin() * drift * dt;
        transform.position.z += (phase * 1.7 + transform.position.y * 0.22).cos() * drift * dt;

        // Wrap within the camera-centered volume on all axes.
        if transform.position.y < floor {
            transform.position.y = ceiling - (floor - transform.position.y) % 4.0;
        }
        for (axis, center) in [(0, camera_position.x), (2, camera_position.z)] {
            let value = if axis == 0 {
                &mut transform.position.x
            } else {
                &mut transform.position.z
            };
            if *value < center - VOLUME_HALF_XZ {
                *value += VOLUME_HALF_XZ * 2.0;
            } else if *value > center + VOLUME_HALF_XZ {
                *value -= VOLUME_HALF_XZ * 2.0;
            }
        }
    }
}

/// Sun dimming factor for the active weather, applied by the render system
/// to the directional light: overcast skies read darker.
pub fn light_factor(resources: &Resources) -> f32 {
    match resources
        .get::<WeatherState>()
        .map(|w| w.active)
        .unwrap_or(WeatherKind::Clear)
    {
        WeatherKind::Rain => 0.45,
        WeatherKind::Snow => 0.62,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_kind_cycle_visits_every_option() {
        let mut kind = WeatherKind::Clear;
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(kind);
            kind = kind.next();
        }
        assert_eq!(kind, WeatherKind::Clear, "cycle must wrap");
        assert!(seen.contains(&WeatherKind::Rain));
        assert!(seen.contains(&WeatherKind::Snow));
        assert!(seen.contains(&WeatherKind::Random));
    }

    #[test]
    fn random_picks_are_deterministic_and_varied() {
        let mut state = WeatherState::new(
            Default::default(),
            Default::default(),
            Default::default(),
            WeatherKind::Random,
        );
        let picks: Vec<WeatherKind> = (0..30).map(|_| state.next_random()).collect();
        assert!(picks.contains(&WeatherKind::Rain));
        assert!(picks.contains(&WeatherKind::Snow));
        assert!(picks.contains(&WeatherKind::Clear));
    }
}
