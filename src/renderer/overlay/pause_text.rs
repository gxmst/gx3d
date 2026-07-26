use crate::game::{PauseMenu, PauseMenuLayout, UiRect, RESOLUTION_OPTIONS};
use wgpu_text::glyph_brush::OwnedSection;

use super::{centered_text_y, scaled_font, section_bounded, OverlayRuntimeInfo};

pub(super) fn build_pause_text_sections(
    menu: &PauseMenu,
    layout: &PauseMenuLayout,
    runtime: OverlayRuntimeInfo,
) -> Vec<OwnedSection> {
    let mut sections = Vec::with_capacity(24);
    let text = [0.91, 0.94, 0.96, 1.0];
    let muted = [0.58, 0.66, 0.71, 1.0];
    let cyan = [0.30, 0.82, 0.86, 1.0];
    let orange = [1.0, 0.68, 0.28, 1.0];
    let violet = [0.72, 0.64, 1.0, 1.0];
    let s = layout.scale;
    let header_pad = (28.0 * s).clamp(19.0, 34.0);

    sections.push(section_bounded(
        layout.header.x + header_pad,
        layout.header.y + (13.0 * s).clamp(9.0, 16.0),
        layout.header.w * 0.65,
        layout.header.h,
        "GXENGINE  /  PHYSICS LAB",
        scaled_font(27.0, s, 20.0, 33.0),
        text,
    ));
    sections.push(section_bounded(
        layout.header.x + header_pad,
        layout.header.y + layout.header.h * 0.56,
        layout.header.w * 0.65,
        layout.header.h * 0.4,
        "暂停菜单 · 实时实验台设置",
        scaled_font(15.0, s, 12.0, 18.0),
        muted,
    ));
    sections.push(section_bounded(
        layout.header.right() - (170.0 * s).clamp(122.0, 205.0),
        layout.header.y + layout.header.h * 0.34,
        (150.0 * s).clamp(110.0, 180.0),
        layout.header.h * 0.5,
        "ESC  返回实验",
        scaled_font(17.0, s, 13.0, 20.0),
        cyan,
    ));

    let panel_pad = (24.0 * s).clamp(16.0, 30.0);
    sections.push(section_bounded(
        layout.panel.x + panel_pad,
        layout.panel.y + (17.0 * s).clamp(11.0, 21.0),
        layout.panel.w - panel_pad * 2.0,
        40.0 * s,
        "显示与体验",
        scaled_font(22.0, s, 17.0, 27.0),
        text,
    ));
    sections.push(section_bounded(
        layout.panel.x + panel_pad,
        layout.resolution_buttons[0].rect.y - (24.0 * s).clamp(17.0, 29.0),
        layout.panel.w - panel_pad * 2.0,
        24.0 * s,
        "窗口分辨率",
        scaled_font(14.0, s, 11.0, 17.0),
        orange,
    ));
    for button in &layout.resolution_buttons {
        let font = scaled_font(17.0, s, 13.0, 20.0);
        let selected = button.index == menu.selected_resolution;
        sections.push(section_bounded(
            button.rect.x + (14.0 * s).clamp(10.0, 17.0),
            centered_text_y(button.rect, font),
            button.rect.w - 24.0 * s,
            button.rect.h,
            if selected {
                format!("●  {}", RESOLUTION_OPTIONS[button.index].label)
            } else {
                RESOLUTION_OPTIONS[button.index].label.to_string()
            },
            font,
            if selected {
                [1.0, 0.90, 0.70, 1.0]
            } else {
                text
            },
        ));
    }

    sections.push(section_bounded(
        layout.language_button.x,
        layout.language_button.y - (24.0 * s).clamp(17.0, 29.0),
        layout.language_button.w,
        24.0 * s,
        "界面语言",
        scaled_font(14.0, s, 11.0, 17.0),
        cyan,
    ));
    let button_font = scaled_font(17.0, s, 13.0, 20.0);
    sections.push(section_bounded(
        layout.language_button.x + (14.0 * s).clamp(10.0, 17.0),
        centered_text_y(layout.language_button, button_font),
        layout.language_button.w - 24.0 * s,
        layout.language_button.h,
        "简体中文  ·  ZH-CN",
        button_font,
        text,
    ));
    sections.push(section_bounded(
        layout.god_mode_button.x,
        layout.god_mode_button.y - (24.0 * s).clamp(17.0, 29.0),
        layout.god_mode_button.w,
        24.0 * s,
        "观察权限",
        scaled_font(14.0, s, 11.0, 17.0),
        if menu.god_mode_enabled { cyan } else { orange },
    ));
    sections.push(section_bounded(
        layout.god_mode_button.x + (14.0 * s).clamp(10.0, 17.0),
        centered_text_y(layout.god_mode_button, button_font),
        layout.god_mode_button.w - 24.0 * s,
        layout.god_mode_button.h,
        if menu.god_mode_enabled {
            "上帝观察模式  ·  已启用"
        } else {
            "上帝观察模式  ·  已禁用"
        },
        button_font,
        text,
    ));
    sections.push(section_bounded(
        layout.sensitivity_button.x,
        layout.sensitivity_button.y - (24.0 * s).clamp(17.0, 29.0),
        layout.sensitivity_button.w,
        24.0 * s,
        "鼠标灵敏度（点击循环，自动保存）",
        scaled_font(14.0, s, 11.0, 17.0),
        violet,
    ));
    sections.push(section_bounded(
        layout.sensitivity_button.x + (14.0 * s).clamp(10.0, 17.0),
        centered_text_y(layout.sensitivity_button, button_font),
        layout.sensitivity_button.w - 24.0 * s,
        layout.sensitivity_button.h,
        format!(
            "灵敏度  ×{:.2}",
            crate::core::config::SENSITIVITY_OPTIONS
                .get(menu.sensitivity_index)
                .copied()
                .unwrap_or(1.0)
        ),
        button_font,
        text,
    ));
    sections.push(section_bounded(
        layout.weather_button.x,
        layout.weather_button.y - (24.0 * s).clamp(17.0, 29.0),
        layout.weather_button.w,
        24.0 * s,
        "天气（点击循环，自动保存）",
        scaled_font(14.0, s, 11.0, 17.0),
        cyan,
    ));
    sections.push(section_bounded(
        layout.weather_button.x + (14.0 * s).clamp(10.0, 17.0),
        centered_text_y(layout.weather_button, button_font),
        layout.weather_button.w - 24.0 * s,
        layout.weather_button.h,
        format!("当前天气  ·  {}", menu.weather.label()),
        button_font,
        text,
    ));
    // Scene selector + session buttons live in the key panel.
    for button in &layout.scene_buttons {
        let font = scaled_font(15.0, s, 12.0, 18.0);
        let current = Some(button.index) == menu.current_scene;
        let name = menu
            .scene_names
            .get(button.index)
            .map(String::as_str)
            .unwrap_or("?");
        sections.push(section_bounded(
            button.rect.x + (10.0 * s).clamp(7.0, 13.0),
            centered_text_y(button.rect, font),
            button.rect.w - 16.0 * s,
            button.rect.h,
            if current {
                format!("● {name}")
            } else {
                name.to_string()
            },
            font,
            if current {
                [1.0, 0.90, 0.70, 1.0]
            } else {
                text
            },
        ));
    }
    let session_font = scaled_font(15.0, s, 12.0, 18.0);
    for (rect, label) in [
        (layout.resume_button, "继续游戏"),
        (layout.reset_button, "重置场景"),
        (layout.quit_button, "退出游戏"),
    ] {
        sections.push(section_bounded(
            rect.x + (12.0 * s).clamp(8.0, 15.0),
            centered_text_y(rect, session_font),
            rect.w - 16.0 * s,
            rect.h,
            label,
            session_font,
            text,
        ));
    }

    let status_pad = (15.0 * s).clamp(10.0, 18.0);
    sections.push(section_bounded(
        layout.status_card.x + status_pad,
        layout.status_card.y + status_pad * 0.7,
        layout.status_card.w - status_pad * 2.0,
        layout.status_card.h - status_pad,
        format!(
            "当前会话  {} · {} · {:.0} FPS\n诊断 {}/3     灯光 {}     动态刚体 {}{}",
            runtime.mode.short_label(),
            runtime.time_label(),
            runtime.fps.max(0.0),
            runtime.enabled_debug_count(),
            runtime.light_count,
            runtime.dynamic_body_count,
            if runtime.holding_object {
                format!("     持物 {:.1}m", runtime.hold_distance)
            } else {
                String::new()
            },
        ),
        scaled_font(14.0, s, 11.0, 17.0),
        muted,
    ));

    sections.push(section_bounded(
        layout.key_panel.x + panel_pad,
        layout.key_panel.y + (17.0 * s).clamp(11.0, 21.0),
        layout.key_panel.w - panel_pad * 2.0,
        40.0 * s,
        "操作指南",
        scaled_font(22.0, s, 17.0, 27.0),
        text,
    ));
    add_key_card_text(
        &mut sections,
        layout.movement_card,
        s,
        "移动与视角",
        "WASD  移动       Shift  奔跑 / 加速\n鼠标  环顾       Space  跳跃 / 上升\nCtrl  下降       V  切换 FPS / GOD",
        cyan,
    );
    add_key_card_text(
        &mut sections,
        layout.interaction_card,
        s,
        "交互与沙盒",
        "左键  射击    右键  瞄准    R  装填\nB  购买武器（1-5 选择）\nE / 中键  抓取或放下    滚轮  调距\nT  投掷    X  冻结    Delete  删除\nG  箱子    N  弹力球    H  重箱",
        orange,
    );
    add_key_card_text(
        &mut sections,
        layout.simulation_card,
        s,
        "时间与诊断",
        "P  暂停       .  单步       [ / ]  倍速\nF3  碰撞体       F4  速度       F5  接触点\nF  按住完整帮助       Esc  设置",
        violet,
    );

    sections.push(section_bounded(
        layout.footer.x + header_pad,
        layout.footer.y + layout.footer.h * 0.28,
        layout.footer.w * 0.72,
        layout.footer.h,
        "提示：分辨率切换保持窗口化并自动居中 · 所有实验状态会继续保留",
        scaled_font(13.0, s, 10.0, 15.0),
        muted,
    ));
    sections
}

fn add_key_card_text(
    sections: &mut Vec<OwnedSection>,
    rect: UiRect,
    scale: f32,
    title: &str,
    body: &str,
    accent: [f32; 4],
) {
    let pad = (16.0 * scale).clamp(10.0, 19.0);
    let title_font = scaled_font(17.0, scale, 13.0, 20.0);
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad * 0.72,
        rect.w - pad * 2.0,
        title_font * 1.4,
        title,
        title_font,
        accent,
    ));
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad + title_font * 1.5,
        rect.w - pad * 2.0,
        rect.h - pad * 2.0 - title_font,
        body,
        scaled_font(14.5, scale, 11.0, 17.0),
        [0.84, 0.89, 0.92, 1.0],
    ));
}
