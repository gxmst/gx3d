//! CS-style team bot match: two bot teams (the player fights on team Alpha)
//! battle over rounds. Bots roam waypoints, engage the nearest visible enemy,
//! push toward last-seen positions, and respawn each round.
//!
//! Round flow: Warmup (freeze, countdown) → Live → RoundOver (banner, pause)
//! → reset to Warmup. First team to `rounds_to_win` wins the match; scores
//! then reset. The player death in match mode does NOT respawn immediately
//! (combat::respawn is bypassed); they spectate until the round ends.

use crate::core::{EngineWorld, Resources, Time, Transform};
use crate::game::{BotBrain, EnemyAI, Team};
use crate::physics::{PhysicsWorld, Ray};
use crate::renderer::Camera;
use glam::Vec3;

use super::{PlayerBody, PlayerHealth, Toast};

/// Row index of the human player in `MatchState::stats`.
pub const PLAYER_STAT: usize = 0;
/// Seconds of holding C required to plant.
const PLANT_SECONDS: f32 = 3.0;
/// Seconds from plant to detonation.
const BOMB_FUSE_SECONDS: f32 = 40.0;
/// Seconds a defender must stay near the bomb to defuse.
const DEFUSE_SECONDS: f32 = 5.0;
/// Defuse proximity radius.
const DEFUSE_RANGE: f32 = 2.6;
/// Detonation damage radius / peak damage.
const BOMB_RADIUS: f32 = 14.0;
const BOMB_DAMAGE: f32 = 160.0;

const BOT_ENGAGE_RANGE: f32 = 34.0;
const BOT_FIRE_INTERVAL: f32 = 1.1;
const BOT_AIM_WARMUP: f32 = 0.45;
const BOT_DAMAGE: f32 = 22.0;
const BOT_MUZZLE_HEIGHT: f32 = 0.6;
/// Seconds bots hold position at round start.
const WARMUP_SECONDS: f32 = 3.0;
/// Seconds the round-over banner stays before the next round.
const ROUND_OVER_SECONDS: f32 = 4.0;
/// How long a bot keeps pushing toward a stale enemy sighting.
const MEMORY_ROAM_RADIUS: f32 = 2.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundPhase {
    Warmup,
    Live,
    RoundOver { winner: Team },
}

/// One scoreboard row.
#[derive(Debug, Clone)]
pub struct StatEntry {
    pub name: String,
    pub team: Team,
    pub kills: u32,
    pub deaths: u32,
}

/// C4 lifecycle within a round.
pub enum BombState {
    Idle,
    Planted {
        position: Vec3,
        /// Seconds until detonation.
        timer: f32,
        /// Accumulated defuse seconds by nearby defenders.
        defuse: f32,
        /// Countdown to the next beep.
        beep_timer: f32,
        /// The glowing bomb prop entity.
        entity: hecs::Entity,
    },
    /// Exploded or defused this round; ignore until reset.
    Resolved,
}

/// Match-wide state resource. Present only in scenes with a `match` block.
pub struct MatchState {
    pub phase: RoundPhase,
    pub phase_timer: f32,
    pub score_alpha: u32,
    pub score_bravo: u32,
    pub rounds_to_win: u32,
    pub round_number: u32,
    /// True while the player is dead and spectating until round end.
    pub player_spectating: bool,
    /// Shared roam waypoints bots move between.
    pub waypoints: Vec<Vec3>,
    /// Player round spawn (first team-A spawn).
    pub player_spawn: Vec3,
    /// Bomb site center + radius; None disables the C4 objective.
    pub bomb_site: Option<(Vec3, f32)>,
    pub bomb: BombState,
    /// Current plant-hold progress in seconds (0 when not planting).
    pub plant_progress: f32,
    /// Scoreboard rows; index 0 is the player.
    pub stats: Vec<StatEntry>,
    rng: u32,
}

impl MatchState {
    pub fn new(
        waypoints: Vec<Vec3>,
        player_spawn: Vec3,
        rounds_to_win: u32,
        bomb_site: Option<(Vec3, f32)>,
        stats: Vec<StatEntry>,
    ) -> Self {
        Self {
            phase: RoundPhase::Warmup,
            phase_timer: WARMUP_SECONDS,
            score_alpha: 0,
            score_bravo: 0,
            rounds_to_win: rounds_to_win.max(1),
            round_number: 1,
            player_spectating: false,
            waypoints,
            player_spawn,
            bomb_site,
            bomb: BombState::Idle,
            plant_progress: 0.0,
            stats,
            rng: 0xB5297A4D,
        }
    }

    /// Scoreboard rows sorted by kills (ties: fewer deaths first).
    pub fn scoreboard_lines(&self) -> Vec<String> {
        let mut rows: Vec<&StatEntry> = self.stats.iter().collect();
        rows.sort_by(|a, b| b.kills.cmp(&a.kills).then(a.deaths.cmp(&b.deaths)));
        rows.iter()
            .map(|entry| {
                let side = match entry.team {
                    Team::Alpha => "[我方]",
                    Team::Bravo => "[敌方]",
                };
                format!(
                    "{side}  {:<14}  击杀 {:>2}   死亡 {:>2}",
                    entry.name, entry.kills, entry.deaths
                )
            })
            .collect()
    }

    fn rand01(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.rng >> 8) as f32 / (u32::MAX >> 8) as f32
    }

    /// Status line for the HUD scoreboard.
    pub fn hud_line(&self) -> String {
        let phase = match self.phase {
            RoundPhase::Warmup => format!("准备  {:.0}s", self.phase_timer.max(0.0).ceil()),
            RoundPhase::Live => "交战中".to_string(),
            RoundPhase::RoundOver { winner } => match winner {
                Team::Alpha => "本回合胜利！".to_string(),
                Team::Bravo => "本回合失守".to_string(),
            },
        };
        let bomb = match &self.bomb {
            BombState::Planted { timer, defuse, .. } if *defuse > 0.3 => {
                format!(" · C4 拆除中 {:.0}%", defuse / DEFUSE_SECONDS * 100.0)
            }
            BombState::Planted { timer, .. } => format!(" · C4 引爆 {:.0}s", timer.max(0.0)),
            _ if self.plant_progress > 0.0 => {
                format!(
                    " · 安装中 {:.0}%",
                    self.plant_progress / PLANT_SECONDS * 100.0
                )
            }
            _ => String::new(),
        };
        format!(
            "第 {} 回合 · 我方 {} : {} 敌方 · {}{}",
            self.round_number, self.score_alpha, self.score_bravo, phase, bomb
        )
    }
}

pub fn fixed_update(world: &mut EngineWorld, resources: &Resources) {
    if !resources.contains::<MatchState>() || super::menu_open(resources) {
        return;
    }
    let dt = resources
        .get::<Time>()
        .map(|t| t.fixed_timestep)
        .unwrap_or(0.0);
    if dt <= 0.0 {
        return;
    }

    // combat::fixed_update owns this decay outside match mode but bows out
    // entirely here, so tick the respawn grace ourselves.
    {
        let mut health = resources.expect_mut::<PlayerHealth>();
        health.invulnerable = (health.invulnerable - dt).max(0.0);
    }

    let phase = {
        let mut state = resources.expect_mut::<MatchState>();
        state.phase_timer -= dt;
        state.phase
    };

    match phase {
        RoundPhase::Warmup => {
            let ready = resources.expect::<MatchState>().phase_timer <= 0.0;
            if ready {
                let mut state = resources.expect_mut::<MatchState>();
                state.phase = RoundPhase::Live;
                drop(state);
                if let Some(mut toast) = resources.get_mut::<Toast>() {
                    toast.show("回合开始！", 1.5);
                }
            }
        }
        RoundPhase::Live => {
            run_bot_combat(world, resources, dt);
            bomb_update(world, resources, dt);
            check_round_end(world, resources);
        }
        RoundPhase::RoundOver { .. } => {
            let ready = resources.expect::<MatchState>().phase_timer <= 0.0;
            if ready {
                start_next_round(world, resources);
            }
        }
    }
}

/// One combat tick: every living bot picks the nearest visible enemy (bot or
/// player), fires with warmup+cooldown, or moves (toward last sighting, else
/// roams waypoints).
fn run_bot_combat(world: &mut EngineWorld, resources: &Resources, dt: f32) {
    let player_position = resources.expect::<Camera>().position;
    let player_body = resources.expect::<PlayerBody>().0;
    let player_alive = !resources.expect::<MatchState>().player_spectating;

    // Snapshot every living bot: (entity, team, position).
    let bots: Vec<(hecs::Entity, Team, Vec3)> = world
        .ecs
        .query::<(hecs::Entity, &BotBrain, &EnemyAI, &Transform)>()
        .iter()
        .filter(|(_, _, ai, _)| ai.is_alive)
        .map(|(entity, brain, _, transform)| (entity, brain.team, transform.position))
        .collect();

    // Damage events applied after the query loop: (target, damage). The player
    // is encoded as None.
    let mut hits: Vec<(Option<hecs::Entity>, f32, usize)> = Vec::new();
    let mut tracers: Vec<(Vec3, Vec3)> = Vec::new();
    let mut muzzle_flashes: Vec<(Vec3, glam::Quat)> = Vec::new();

    {
        let physics = resources.expect::<PhysicsWorld>();
        for (entity, brain, ai, transform) in world
            .ecs
            .query::<(hecs::Entity, &mut BotBrain, &mut EnemyAI, &mut Transform)>()
            .iter()
        {
            if !ai.is_alive {
                continue;
            }
            ai.fire_cooldown = (ai.fire_cooldown - dt).max(0.0);
            let muzzle = transform.position + Vec3::Y * BOT_MUZZLE_HEIGHT;

            // Candidate targets: enemy bots + the player (if opposing Alpha).
            let mut best: Option<(Vec3, f32, Option<hecs::Entity>)> = None;
            for (other_entity, other_team, other_position) in &bots {
                if *other_team == brain.team || *other_entity == entity {
                    continue;
                }
                let aim = *other_position + Vec3::Y * BOT_MUZZLE_HEIGHT;
                let distance = muzzle.distance(aim);
                if distance > BOT_ENGAGE_RANGE {
                    continue;
                }
                if best.map(|(_, d, _)| distance < d).unwrap_or(true)
                    && sees_point(&physics, muzzle, aim, Some(*other_entity))
                {
                    best = Some((aim, distance, Some(*other_entity)));
                }
            }
            if brain.team == Team::Bravo && player_alive {
                let distance = muzzle.distance(player_position);
                if distance <= BOT_ENGAGE_RANGE
                    && best.map(|(_, d, _)| distance < d).unwrap_or(true)
                    && sees_player(&physics, muzzle, player_position, player_body)
                {
                    best = Some((player_position, distance, None));
                }
            }

            if let Some((target_pos, distance, target_entity)) = best {
                brain.last_seen_enemy = Some(target_pos);
                // Face the target and hold position while engaging.
                let flat = Vec3::new(
                    target_pos.x - transform.position.x,
                    0.0,
                    target_pos.z - transform.position.z,
                );
                if flat.length_squared() > 1.0e-4 {
                    transform.rotation = glam::Quat::from_rotation_arc(Vec3::Z, flat.normalize());
                }
                ai.aim_warmup += dt;
                if ai.aim_warmup >= BOT_AIM_WARMUP && ai.fire_cooldown <= 0.0 {
                    ai.fire_cooldown = BOT_FIRE_INTERVAL;
                    tracers.push((muzzle, target_pos));
                    muzzle_flashes.push((transform.position, transform.rotation));
                    // Deterministic accuracy roll, harder at range.
                    let accuracy = (1.0 - distance / BOT_ENGAGE_RANGE).clamp(0.0, 1.0) * 0.5 + 0.3;
                    let roll = (muzzle.dot(Vec3::new(12.9898, 78.233, 37.719)).sin() * 43_758.547)
                        .fract()
                        .abs();
                    if roll < accuracy {
                        hits.push((target_entity, BOT_DAMAGE, brain.stat_index));
                    }
                }
                // Clear roam target so movement resumes fresh after combat.
                ai.waypoints.clear();
            } else {
                ai.aim_warmup = 0.0;
                // No target visible: push toward the last sighting, else roam.
                let need_new_goal = ai.waypoints.is_empty()
                    || transform
                        .position
                        .distance(*ai.waypoints.first().unwrap_or(&transform.position))
                        < MEMORY_ROAM_RADIUS;
                if need_new_goal {
                    let goal = brain.last_seen_enemy.take().or_else(|| {
                        let mut state = resources.expect_mut::<MatchState>();
                        // A planted bomb overrides roaming for both sides:
                        // defenders rush to defuse, attackers fall back to hold.
                        if let BombState::Planted { position, .. } = state.bomb {
                            return Some(position);
                        }
                        if state.waypoints.is_empty() {
                            None
                        } else {
                            let index = (state.rand01() * state.waypoints.len() as f32) as usize;
                            state.waypoints.get(index).copied()
                        }
                    });
                    if let Some(goal) = goal {
                        ai.waypoints = vec![goal];
                        ai.current_waypoint = 0;
                    }
                }
            }
        }
    }

    for (from, to) in tracers {
        spawn_bot_tracer(world, resources, from, to + Vec3::Y * 0.1);
    }
    if !muzzle_flashes.is_empty() {
        if let Some(mut flashes) = resources.get_mut::<super::bot_weapon::BotShotFlashes>() {
            flashes.0.extend(muzzle_flashes);
        }
    }

    let mut player_damage = 0.0;
    let mut player_shooter = 0usize;
    for (target, damage, shooter_stat) in hits {
        match target {
            Some(entity) => {
                let mut killed = false;
                if let Ok(mut ai) = world.ecs.get::<&mut EnemyAI>(entity) {
                    let was_alive = ai.is_alive;
                    ai.take_damage(damage);
                    ai.stagger_timer = 0.2;
                    killed = was_alive && !ai.is_alive;
                }
                if killed {
                    let victim_stat = world
                        .ecs
                        .get::<&BotBrain>(entity)
                        .map(|brain| brain.stat_index)
                        .ok();
                    let mut state = resources.expect_mut::<MatchState>();
                    if let Some(entry) = state.stats.get_mut(shooter_stat) {
                        entry.kills += 1;
                    }
                    if let Some(victim) = victim_stat {
                        if let Some(entry) = state.stats.get_mut(victim) {
                            entry.deaths += 1;
                        }
                    }
                }
            }
            None => {
                player_damage += damage;
                player_shooter = shooter_stat;
            }
        }
    }
    if player_damage > 0.0 {
        damage_player_in_match(resources, player_damage, player_shooter);
    }
}

/// C4 objective tick: player planting, fuse countdown + beeps, bot defusing,
/// detonation.
fn bomb_update(world: &mut EngineWorld, resources: &Resources, dt: f32) {
    let Some((site_center, site_radius)) = resources.expect::<MatchState>().bomb_site else {
        return;
    };

    // --- Planting: hold C inside the site while alive. ---
    let planting_possible = {
        let state = resources.expect::<MatchState>();
        matches!(state.bomb, BombState::Idle) && !state.player_spectating
    };
    if planting_possible {
        let player_position = resources.expect::<Camera>().position;
        let in_site = Vec3::new(
            player_position.x - site_center.x,
            0.0,
            player_position.z - site_center.z,
        )
        .length()
            < site_radius;
        let holding_c = resources
            .get::<crate::input::InputState>()
            .map(|input| input.is_key_pressed(winit::keyboard::KeyCode::KeyC))
            .unwrap_or(false);
        let mut state = resources.expect_mut::<MatchState>();
        if in_site && holding_c {
            state.plant_progress += dt;
            if state.plant_progress >= PLANT_SECONDS {
                state.plant_progress = 0.0;
                drop(state);
                let position = Vec3::new(player_position.x, site_center.y, player_position.z);
                let entity = spawn_bomb_prop(world, resources, position);
                let mut state = resources.expect_mut::<MatchState>();
                state.bomb = BombState::Planted {
                    position,
                    timer: BOMB_FUSE_SECONDS,
                    defuse: 0.0,
                    beep_timer: 0.0,
                    entity,
                };
                drop(state);
                if let Some(mut toast) = resources.get_mut::<Toast>() {
                    toast.show(format!("C4 已安装！{BOMB_FUSE_SECONDS:.0} 秒后引爆"), 3.0);
                }
            }
        } else {
            state.plant_progress = 0.0;
        }
    }

    // --- Fuse / defuse. ---
    let planted = {
        let state = resources.expect::<MatchState>();
        match state.bomb {
            BombState::Planted {
                position,
                timer,
                defuse,
                beep_timer,
                entity,
            } => Some((position, timer, defuse, beep_timer, entity)),
            _ => None,
        }
    };
    let Some((position, mut timer, mut defuse, mut beep_timer, entity)) = planted else {
        return;
    };
    timer -= dt;

    // Beep cadence accelerates as the fuse runs down.
    beep_timer -= dt;
    if beep_timer <= 0.0 {
        beep_timer = 0.15 + (timer / BOMB_FUSE_SECONDS).clamp(0.0, 1.0) * 0.85;
        if let Some(mut audio) = resources.get_mut::<Option<crate::audio::AudioSystem>>() {
            if let Some(ref mut audio) = *audio {
                audio.play_beep();
            }
        }
    }

    // Any living defender close to the bomb defuses; progress resets when
    // nobody is on the kit.
    let defender_near = world
        .ecs
        .query::<(&BotBrain, &EnemyAI, &Transform)>()
        .iter()
        .any(|(brain, ai, transform)| {
            brain.team == Team::Bravo
                && ai.is_alive
                && transform.position.distance(position) < DEFUSE_RANGE
        });
    if defender_near {
        defuse += dt;
    } else {
        defuse = 0.0;
    }

    if defuse >= DEFUSE_SECONDS {
        world.despawn(entity);
        {
            let mut state = resources.expect_mut::<MatchState>();
            state.bomb = BombState::Resolved;
        }
        if let Some(mut toast) = resources.get_mut::<Toast>() {
            toast.show("C4 被拆除！", 2.5);
        }
        award_round(resources, Team::Bravo);
        return;
    }

    if timer <= 0.0 {
        world.despawn(entity);
        detonate_bomb(world, resources, position);
        {
            let mut state = resources.expect_mut::<MatchState>();
            state.bomb = BombState::Resolved;
        }
        award_round(resources, Team::Alpha);
        return;
    }

    let mut state = resources.expect_mut::<MatchState>();
    state.bomb = BombState::Planted {
        position,
        timer,
        defuse,
        beep_timer,
        entity,
    };
}

/// The armed C4 prop: a small glowing box at the plant spot.
fn spawn_bomb_prop(world: &mut EngineWorld, resources: &Resources, position: Vec3) -> hecs::Entity {
    let entity = world.spawn();
    world.add_component(
        entity,
        Transform::new(
            position + Vec3::Y * 0.12,
            glam::Quat::from_rotation_y(0.6),
            Vec3::new(0.34, 0.22, 0.24),
        ),
    );
    if let (Some(sandbox), Some(assets)) = (
        resources.get::<crate::physics::PhysicsSandbox>(),
        resources.get::<super::WeaponFeedbackAssets>(),
    ) {
        world.add_component(
            entity,
            crate::renderer::RenderMesh {
                mesh: sandbox.cube_mesh,
                material: assets.muzzle_flash_material,
            },
        );
    }
    entity
}

/// Detonation: heavy radial damage to bots and the player, big glow burst.
fn detonate_bomb(world: &mut EngineWorld, resources: &Resources, origin: Vec3) {
    if let Some(mut audio) = resources.get_mut::<Option<crate::audio::AudioSystem>>() {
        if let Some(ref mut audio) = *audio {
            audio.play_explosion();
        }
    }
    // Bots.
    for (ai, transform) in world.ecs.query::<(&mut EnemyAI, &Transform)>().iter() {
        let distance = transform.position.distance(origin);
        if distance < BOMB_RADIUS && ai.is_alive {
            ai.take_damage(BOMB_DAMAGE * (1.0 - distance / BOMB_RADIUS).max(0.15));
        }
    }
    // Player.
    let player_distance = resources.expect::<Camera>().position.distance(origin);
    if player_distance < BOMB_RADIUS {
        damage_player_in_match(
            resources,
            BOMB_DAMAGE * (1.0 - player_distance / BOMB_RADIUS).max(0.15),
            PLAYER_STAT,
        );
    }
    // Glow burst (bigger sibling of the enemy death pop).
    if let Some(assets) = resources.get::<super::WeaponFeedbackAssets>().map(|a| *a) {
        for i in 0..14 {
            let angle = i as f32 * 0.449;
            let radius = 0.4 + (i % 5) as f32 * 0.9;
            let burst = world.spawn();
            world.add_component(
                burst,
                Transform::new(
                    origin
                        + Vec3::new(
                            angle.cos() * radius,
                            0.3 + (i % 4) as f32 * 0.55,
                            angle.sin() * radius,
                        ),
                    glam::Quat::IDENTITY,
                    Vec3::splat(0.5 - (i % 5) as f32 * 0.06),
                ),
            );
            world.add_component(
                burst,
                crate::renderer::RenderMesh {
                    mesh: assets.impact_mesh,
                    material: assets.impact_material,
                },
            );
            world.add_component(
                burst,
                super::TimedEffect {
                    remaining: 0.3 + (i % 3) as f32 * 0.12,
                },
            );
        }
    }
}

fn sees_point(
    physics: &PhysicsWorld,
    from: Vec3,
    to: Vec3,
    target_entity: Option<hecs::Entity>,
) -> bool {
    let offset = to - from;
    let distance = offset.length();
    if distance < 0.1 {
        return true;
    }
    let ray = Ray::new(from, offset, distance + 0.4);
    physics
        .cast_ray(&ray)
        .map(|hit| hit.entity == target_entity || hit.distance >= distance - 0.6)
        .unwrap_or(true)
}

fn sees_player(
    physics: &PhysicsWorld,
    from: Vec3,
    player_position: Vec3,
    player_body: rapier3d::prelude::RigidBodyHandle,
) -> bool {
    let offset = player_position - from;
    let ray = Ray::new(from, offset, offset.length() + 0.5);
    physics
        .cast_ray(&ray)
        .and_then(|hit| physics.collider_body(hit.collider_handle))
        .map(|body| body == player_body)
        .unwrap_or(false)
}

/// Player damage in match mode: at zero they spectate until round end
/// (no instant respawn — that is combat::respawn_player's job outside
/// match mode).
fn damage_player_in_match(resources: &Resources, damage: f32, shooter_stat: usize) {
    let died = {
        let mut health = resources.expect_mut::<PlayerHealth>();
        if health.invulnerable > 0.0 {
            return;
        }
        health.current = (health.current - damage).max(0.0);
        health.hurt_flash = 1.0;
        health.current <= 0.0
    };
    if let Some(mut audio) = resources.get_mut::<Option<crate::audio::AudioSystem>>() {
        if let Some(ref mut audio) = *audio {
            audio.play_hurt();
        }
    }
    if died {
        let mut state = resources.expect_mut::<MatchState>();
        if !state.player_spectating {
            state.player_spectating = true;
            if let Some(entry) = state.stats.get_mut(PLAYER_STAT) {
                entry.deaths += 1;
            }
            if shooter_stat != PLAYER_STAT {
                if let Some(entry) = state.stats.get_mut(shooter_stat) {
                    entry.kills += 1;
                }
            }
            drop(state);
            if let Some(mut toast) = resources.get_mut::<Toast>() {
                toast.show("你阵亡了 · 观战至回合结束", 3.0);
            }
            // Free-fly spectator view while dead.
            if let Some(mut view) = resources.get_mut::<super::ViewModeState>() {
                view.mode = super::ViewMode::God;
            }
        }
    }
}

/// A round ends when one side has no living members.
fn check_round_end(world: &mut EngineWorld, resources: &Resources) {
    let mut alpha_alive = 0;
    let mut bravo_alive = 0;
    for (brain, ai) in world.ecs.query::<(&BotBrain, &EnemyAI)>().iter() {
        if ai.is_alive {
            match brain.team {
                Team::Alpha => alpha_alive += 1,
                Team::Bravo => bravo_alive += 1,
            }
        }
    }
    let player_alive = !resources.expect::<MatchState>().player_spectating;
    if player_alive {
        alpha_alive += 1;
    }

    // With the bomb planted, wiping team Alpha does NOT end the round —
    // the fuse decides unless the defenders defuse it in time. Wiping the
    // defenders always ends it (nobody left to defuse).
    let bomb_planted = matches!(
        resources.expect::<MatchState>().bomb,
        BombState::Planted { .. }
    );
    let winner = if bravo_alive == 0 {
        Some(Team::Alpha)
    } else if alpha_alive == 0 && !bomb_planted {
        Some(Team::Bravo)
    } else {
        None
    };
    let Some(winner) = winner else { return };
    award_round(resources, winner);
}

/// Score the round for `winner`, hand out economy rewards, announce.
fn award_round(resources: &Resources, winner: Team) {
    if matches!(
        resources.expect::<MatchState>().phase,
        RoundPhase::RoundOver { .. }
    ) {
        return;
    }
    let mut state = resources.expect_mut::<MatchState>();
    match winner {
        Team::Alpha => state.score_alpha += 1,
        Team::Bravo => state.score_bravo += 1,
    }
    state.phase = RoundPhase::RoundOver { winner };
    state.phase_timer = ROUND_OVER_SECONDS;
    drop(state);
    // Round economy: win/loss rewards.
    if let Some(mut buy) = resources.get_mut::<super::buy_menu::BuyState>() {
        buy.award(match winner {
            Team::Alpha => super::buy_menu::ROUND_WIN_REWARD,
            Team::Bravo => super::buy_menu::ROUND_LOSS_REWARD,
        });
    }
    let state = resources.expect_mut::<MatchState>();

    // Match point: announce and reset the series.
    let (a, b, needed) = (state.score_alpha, state.score_bravo, state.rounds_to_win);
    drop(state);
    if let Some(mut toast) = resources.get_mut::<Toast>() {
        if a >= needed {
            toast.show(format!("比赛胜利！ {a} : {b}"), 4.0);
        } else if b >= needed {
            toast.show(format!("比赛失败 {a} : {b}"), 4.0);
        } else {
            match winner {
                Team::Alpha => toast.show("回合胜利！", 2.5),
                Team::Bravo => toast.show("回合失守", 2.5),
            }
        }
    }
}

/// Reset all actors for the next round (or a fresh match after match point).
fn start_next_round(world: &mut EngineWorld, resources: &Resources) {
    // Clear any leftover bomb prop and objective state.
    {
        let bomb_entity = {
            let state = resources.expect::<MatchState>();
            match state.bomb {
                BombState::Planted { entity, .. } => Some(entity),
                _ => None,
            }
        };
        if let Some(entity) = bomb_entity {
            world.despawn(entity);
        }
        let mut state = resources.expect_mut::<MatchState>();
        state.bomb = BombState::Idle;
        state.plant_progress = 0.0;
    }
    // Series over? Reset scores.
    {
        let mut state = resources.expect_mut::<MatchState>();
        if state.score_alpha >= state.rounds_to_win || state.score_bravo >= state.rounds_to_win {
            state.score_alpha = 0;
            state.score_bravo = 0;
            state.round_number = 0;
        }
        state.round_number += 1;
        state.phase = RoundPhase::Warmup;
        state.phase_timer = WARMUP_SECONDS;
        state.player_spectating = false;
    }

    // Revive & reposition every bot at its home spawn.
    let bots: Vec<(hecs::Entity, Vec3)> = world
        .ecs
        .query::<(hecs::Entity, &BotBrain, &EnemyAI)>()
        .iter()
        .map(|(entity, brain, _)| (entity, brain.home))
        .collect();
    {
        let mut physics = resources.expect_mut::<PhysicsWorld>();
        for (entity, home) in &bots {
            if let Ok(mut ai) = world.ecs.get::<&mut EnemyAI>(*entity) {
                ai.health = ai.max_health;
                ai.is_alive = true;
                ai.waypoints.clear();
                ai.aim_warmup = 0.0;
                ai.fire_cooldown = 0.0;
            }
            if let Ok(mut transform) = world.ecs.get::<&mut Transform>(*entity) {
                transform.position = *home;
            }
            if let Ok(body) = world.ecs.get::<&crate::physics::PhysicsBody>(*entity) {
                let handle = body.rigid_body_handle;
                drop(body);
                physics.teleport_body(handle, *home);
            }
            if let Ok(mut brain) = world.ecs.get::<&mut BotBrain>(*entity) {
                brain.last_seen_enemy = None;
            }
        }

        // Player back to spawn, restored, out of spectator mode.
        let spawn = resources.expect::<MatchState>().player_spawn;
        let player_body = resources.expect::<PlayerBody>().0;
        physics.teleport_body(player_body, spawn);
    }
    {
        let mut player = resources.expect_mut::<crate::game::Player>();
        player.reset_motion();
    }
    {
        let mut health = resources.expect_mut::<PlayerHealth>();
        health.current = health.max;
        health.invulnerable = 1.0;
    }
    if let Some(mut view) = resources.get_mut::<super::ViewModeState>() {
        view.mode = super::ViewMode::Fps;
    }
    // Fresh round, fresh sidearm: back to the default pistol like a CS
    // pistol-round; buy better gear with B during warmup.
    super::buy_menu::reset_to_default_weapon(resources);
}

/// Bots may only move during Live; freezes patrol movement in other phases.
pub fn movement_allowed(resources: &Resources) -> bool {
    resources
        .get::<MatchState>()
        .map(|state| state.phase == RoundPhase::Live)
        .unwrap_or(true)
}

fn spawn_bot_tracer(world: &mut EngineWorld, resources: &Resources, from: Vec3, to: Vec3) {
    let Some(assets) = resources.get::<super::WeaponFeedbackAssets>().map(|a| *a) else {
        return;
    };
    let offset = to - from;
    let length = offset.length();
    if length < 0.5 {
        return;
    }
    let rotation = glam::Quat::from_rotation_arc(Vec3::Z, offset / length);
    let tracer = world.spawn();
    world.add_component(
        tracer,
        Transform::new(
            from + offset * 0.5,
            rotation,
            Vec3::new(0.025, 0.025, length),
        ),
    );
    world.add_component(
        tracer,
        crate::renderer::RenderMesh {
            mesh: assets.impact_mesh,
            material: assets.muzzle_flash_material,
        },
    );
    world.add_component(tracer, super::TimedEffect { remaining: 0.06 });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_line_reflects_phase_and_score() {
        let mut state = MatchState::new(Vec::new(), Vec3::ZERO, 5, None, Vec::new());
        state.score_alpha = 2;
        state.score_bravo = 1;
        assert!(state.hud_line().contains("2 : 1"));
        state.phase = RoundPhase::Live;
        assert!(state.hud_line().contains("交战中"));
    }

    #[test]
    fn team_opponent_is_symmetric() {
        assert_eq!(Team::Alpha.opponent(), Team::Bravo);
        assert_eq!(Team::Bravo.opponent(), Team::Alpha);
    }
}
