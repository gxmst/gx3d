//! The F1 debug panel: live sliders and toggles over the engine's tunable
//! state. This is DESIGN.md's "实时把玩参数" tool — no recompiles, no JSON
//! edits, no hunting for spare hotkeys.
//!
//! Widget code lives here (game layer); the egui plumbing lives in
//! `renderer::debug_ui` and stays gameplay-agnostic.

use crate::core::{Resources, Time};
use crate::game::systems::weather::WeatherKind;
use crate::game::Tornado;
use crate::physics::PhysicsWorld;
use glam::Vec3;

/// Whether the panel is open (toggled by F1 in the input path).
#[derive(Default)]
pub struct DebugPanelState {
    pub open: bool,
}

/// Build the panel widgets. Called from the render system while the egui
/// frame is open; mutates resources directly so every slider is live.
pub fn build(ui: &mut egui::Ui, world: &mut crate::core::EngineWorld, resources: &Resources) {
    egui::Window::new("调试面板  ·  F1 关闭")
        .default_width(320.0)
        .resizable(true)
        .show(ui.ctx(), |ui| {
            time_section(ui, resources);
            physics_section(ui, resources);
            render_section(ui, resources);
            weather_section(ui, resources);
            effects_section(ui, world, resources);
            layers_section(ui, resources);
        });
}

fn time_section(ui: &mut egui::Ui, resources: &Resources) {
    egui::CollapsingHeader::new("时间")
        .default_open(true)
        .show(ui, |ui| {
            let Some(mut time) = resources.get_mut::<Time>() else {
                return;
            };
            let mut scale = time.time_scale;
            ui.horizontal(|ui| {
                ui.label("倍速");
                if ui
                    .add(egui::Slider::new(&mut scale, 0.0..=4.0).logarithmic(false))
                    .changed()
                {
                    time.set_time_scale(scale);
                }
            });
            ui.horizontal(|ui| {
                for (label, value) in [
                    ("暂停", 0.0),
                    ("0.1×", 0.1),
                    ("0.25×", 0.25),
                    ("1×", 1.0),
                    ("2×", 2.0),
                ] {
                    if ui.button(label).clicked() {
                        time.set_time_scale(value);
                    }
                }
                if ui.button("单步").clicked() {
                    time.request_single_step();
                }
            });
        });
}

fn physics_section(ui: &mut egui::Ui, resources: &Resources) {
    egui::CollapsingHeader::new("物理")
        .default_open(true)
        .show(ui, |ui| {
            let Some(mut physics) = resources.get_mut::<PhysicsWorld>() else {
                return;
            };
            ui.horizontal(|ui| {
                ui.label("重力 Y");
                ui.add(egui::Slider::new(&mut physics.gravity.y, -30.0..=10.0));
            });
            ui.horizontal(|ui| {
                if ui.button("月球 (-1.6)").clicked() {
                    physics.gravity = Vec3::new(0.0, -1.6, 0.0);
                }
                if ui.button("地球 (-9.8)").clicked() {
                    physics.gravity = Vec3::new(0.0, -9.81, 0.0);
                }
                if ui.button("木星 (-24.8)").clicked() {
                    physics.gravity = Vec3::new(0.0, -24.8, 0.0);
                }
                if ui.button("无重力").clicked() {
                    physics.gravity = Vec3::ZERO;
                }
            });
            let dynamic = physics
                .rigid_body_set
                .iter()
                .filter(|(_, body)| body.is_dynamic())
                .count();
            ui.label(format!(
                "动态刚体 {dynamic} · 冲量事件 {}",
                physics.impulse_events.len()
            ));
        });
}

fn render_section(ui: &mut egui::Ui, resources: &Resources) {
    egui::CollapsingHeader::new("渲染").show(ui, |ui| {
        let Some(mut exposure) = resources.get_mut::<super::render::ExposureSetting>() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label("曝光");
            ui.add(egui::Slider::new(&mut exposure.0, 0.2..=3.0));
        });
        if ui.button("重置曝光").clicked() {
            exposure.0 = 1.0;
        }
    });
}

fn weather_section(ui: &mut egui::Ui, resources: &Resources) {
    egui::CollapsingHeader::new("天气").show(ui, |ui| {
        let Some(mut weather) = resources.get_mut::<super::weather::WeatherState>() else {
            return;
        };
        ui.horizontal(|ui| {
            for (label, kind) in [
                ("晴", WeatherKind::Clear),
                ("雨", WeatherKind::Rain),
                ("雪", WeatherKind::Snow),
                ("随机", WeatherKind::Random),
            ] {
                if ui
                    .selectable_label(weather.selected == kind, label)
                    .clicked()
                {
                    weather.selected = kind;
                }
            }
        });
        ui.label(format!("当前生效：{}", weather.active.label()));
    });
}

fn effects_section(ui: &mut egui::Ui, world: &mut crate::core::EngineWorld, resources: &Resources) {
    egui::CollapsingHeader::new("特效").show(ui, |ui| {
        // Tornado tuning: the showcase scene has one; sliders no-op elsewhere.
        let mut any_tornado = false;
        for tornado in world.ecs.query::<&mut Tornado>().iter() {
            any_tornado = true;
            ui.label("龙卷风");
            ui.horizontal(|ui| {
                ui.label("强度");
                ui.add(egui::Slider::new(&mut tornado.strength, 0.0..=1200.0));
            });
            ui.horizontal(|ui| {
                ui.label("半径");
                ui.add(egui::Slider::new(&mut tornado.radius, 2.0..=40.0));
            });
        }
        if !any_tornado {
            ui.label("（本场景无龙卷风）");
        }
        if let Some(mut water) = resources.get_mut::<Option<crate::game::WaterSurface>>() {
            if let Some(water) = water.as_mut() {
                ui.label("水面");
                ui.horizontal(|ui| {
                    ui.label("浪高");
                    ui.add(egui::Slider::new(&mut water.amplitude, 0.0..=2.5));
                });
            }
        }
        if ui.button("生成布娃娃 (K)").clicked() {
            let (origin, forward) = {
                let camera = resources.expect::<crate::renderer::Camera>();
                (camera.position, camera.forward())
            };
            super::ragdoll::spawn_ragdoll(world, resources, origin + forward * 3.0);
        }
    });
}

fn layers_section(ui: &mut egui::Ui, resources: &Resources) {
    egui::CollapsingHeader::new("可视化图层").show(ui, |ui| {
        let Some(mut state) = resources.get_mut::<super::PhysicsDebugState>() else {
            return;
        };
        ui.checkbox(&mut state.colliders, "碰撞体线框 (F3)");
        ui.checkbox(&mut state.velocities, "速度矢量 (F4)");
        ui.checkbox(&mut state.contacts, "接触点 (F5)");
        ui.checkbox(&mut state.impulses, "冲量箭头 (F6)");
    });
}
