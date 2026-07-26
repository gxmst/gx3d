//! Headless rules coverage for the match/bomb state machines: the pieces
//! that are pure data transitions and must not silently regress.

use glam::Vec3;
use gxengine::game::systems::match_mode::{BombState, MatchState, RoundPhase, StatEntry};
use gxengine::game::Team;

fn stats() -> Vec<StatEntry> {
    vec![
        StatEntry {
            name: "你".into(),
            team: Team::Alpha,
            kills: 0,
            deaths: 0,
        },
        StatEntry {
            name: "敌方 1".into(),
            team: Team::Bravo,
            kills: 0,
            deaths: 0,
        },
    ]
}

#[test]
fn new_match_starts_in_warmup_round_one() {
    let state = MatchState::new(Vec::new(), Vec3::ZERO, 5, None, stats());
    assert_eq!(state.phase, RoundPhase::Warmup);
    assert_eq!(state.round_number, 1);
    assert_eq!(state.score_alpha, 0);
    assert_eq!(state.score_bravo, 0);
    assert!(matches!(state.bomb, BombState::Idle));
    assert!(!state.player_spectating);
}

#[test]
fn hud_line_shows_bomb_countdown_when_planted() {
    let mut state = MatchState::new(Vec::new(), Vec3::ZERO, 5, Some((Vec3::ZERO, 6.0)), stats());
    state.phase = RoundPhase::Live;
    // A fake planted bomb: entity id is irrelevant for the HUD line.
    let mut world = gxengine::core::EngineWorld::new();
    let entity = world.spawn();
    state.bomb = BombState::Planted {
        position: Vec3::ZERO,
        timer: 27.4,
        defuse: 0.0,
        beep_timer: 0.0,
        entity,
    };
    let line = state.hud_line();
    assert!(
        line.contains("C4"),
        "hud line should mention the bomb: {line}"
    );
    assert!(line.contains("27") || line.contains("28"), "{line}");
}

#[test]
fn hud_line_shows_defuse_progress_over_countdown() {
    let mut state = MatchState::new(Vec::new(), Vec3::ZERO, 5, Some((Vec3::ZERO, 6.0)), stats());
    state.phase = RoundPhase::Live;
    let mut world = gxengine::core::EngineWorld::new();
    let entity = world.spawn();
    state.bomb = BombState::Planted {
        position: Vec3::ZERO,
        timer: 20.0,
        defuse: 2.5,
        beep_timer: 0.0,
        entity,
    };
    let line = state.hud_line();
    assert!(line.contains("拆除中"), "{line}");
}

#[test]
fn scoreboard_sorts_by_kills_then_deaths() {
    let mut state = MatchState::new(Vec::new(), Vec3::ZERO, 5, None, stats());
    state.stats[0].kills = 1;
    state.stats[0].deaths = 3;
    state.stats[1].kills = 1;
    state.stats[1].deaths = 1;
    let lines = state.scoreboard_lines();
    // Same kills: fewer deaths ranks first.
    assert!(lines[0].contains("敌方 1"), "{lines:?}");
    assert!(lines[1].contains("你"), "{lines:?}");
}

#[test]
fn warmup_phase_reports_countdown() {
    let state = MatchState::new(Vec::new(), Vec3::ZERO, 5, None, stats());
    assert!(state.hud_line().contains("准备"));
}
