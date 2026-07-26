use crate::game::UiRect;
use wgpu_text::glyph_brush::OwnedSection;

use super::layout::{debug_badge_rects, HudLayout};
use super::primitives::{add_card, add_outline, add_rect, OverlayVertex};
use super::{centered_text_y, scaled_font, section_bounded, HudState};

pub(super) fn build_hud_text_sections(
    state: &HudState<'_>,
    layout: &HudLayout,
    screen_width: f32,
    screen_height: f32,
) -> Vec<OwnedSection> {
    let mut sections = Vec::with_capacity(18);
    let text = [0.91, 0.95, 0.97, 1.0];
    let muted = [0.60, 0.68, 0.73, 1.0];
    let s = layout.scale;
    let pad = (16.0 * s).clamp(11.0, 19.0);

    sections.push(section_bounded(
        layout.mode_panel.x + pad,
        layout.mode_panel.y + (10.0 * s).clamp(7.0, 12.0),
        layout.mode_panel.w - pad * 2.0,
        layout.mode_panel.h * 0.52,
        format!(
            "{}  /  {}",
            state.runtime.mode.short_label(),
            state.runtime.mode.description()
        ),
        scaled_font(19.0, s, 15.0, 23.0),
        text,
    ));
    sections.push(section_bounded(
        layout.mode_panel.x + pad,
        layout.mode_panel.y + layout.mode_panel.h * 0.58,
        layout.mode_panel.w - pad * 2.0,
        layout.mode_panel.h * 0.34,
        state.runtime.time_label(),
        scaled_font(13.0, s, 11.0, 16.0),
        if state.runtime.paused {
            [1.0, 0.48, 0.38, 1.0]
        } else {
            [0.42, 0.82, 0.86, 1.0]
        },
    ));

    sections.push(section_bounded(
        layout.telemetry_panel.x + pad,
        layout.telemetry_panel.y + (9.0 * s).clamp(6.0, 11.0),
        layout.telemetry_panel.w - pad * 2.0,
        layout.telemetry_panel.h * 0.44,
        format!(
            "{:.0} FPS  ·  {} LIGHTS  ·  {} BODIES",
            state.runtime.fps.max(0.0),
            state.runtime.light_count,
            state.runtime.dynamic_body_count
        ),
        scaled_font(17.0, s, 13.0, 21.0),
        [0.44, 0.86, 0.89, 1.0],
    ));
    sections.push(section_bounded(
        layout.telemetry_panel.x + pad,
        layout.telemetry_panel.y + layout.telemetry_panel.h * 0.56,
        layout.telemetry_panel.w - pad * 2.0,
        layout.telemetry_panel.h * 0.35,
        format!(
            "POS  {:+.1}  {:+.1}  {:+.1}",
            state.runtime.camera_position[0],
            state.runtime.camera_position[1],
            state.runtime.camera_position[2]
        ),
        scaled_font(13.0, s, 10.5, 16.0),
        muted,
    ));

    for ((rect, label), enabled) in debug_badge_rects(layout)
        .into_iter()
        .zip(["F3 碰撞", "F4 速度", "F5 接触", "F6 冲量"])
        .zip([
            state.runtime.debug_colliders,
            state.runtime.debug_velocities,
            state.runtime.debug_contacts,
            state.runtime.debug_impulses,
        ])
    {
        let font = scaled_font(12.5, s, 10.0, 15.0);
        sections.push(section_bounded(
            rect.x + (7.0 * s).clamp(5.0, 9.0),
            centered_text_y(rect, font),
            rect.w - 12.0 * s,
            rect.h,
            label,
            font,
            if enabled { text } else { muted },
        ));
    }

    sections.push(section_bounded(
        layout.hint_panel.x + pad,
        layout.hint_panel.y + (8.0 * s).clamp(6.0, 10.0),
        layout.hint_panel.w - pad * 2.0,
        layout.hint_panel.h,
        if state.runtime.holding_object {
            format!(
                "持物中  {:.1}m     ·     滚轮  调距\nT  投掷     X  冻结     E / 中键  放下",
                state.runtime.hold_distance
            )
        } else if state.first_time {
            "新手提示：按住 F 查看完整操作指南\nESC  菜单 / 切换场景 · 看过一次后此提示消失"
                .to_string()
        } else {
            "F  操作指南   ·   ESC  菜单   ·   B  买枪\nG / N / H  生成     E  抓取     T  推动"
                .to_string()
        },
        scaled_font(13.0, s, 10.5, 16.0),
        if state.first_time {
            [1.0, 0.82, 0.40, 1.0]
        } else {
            [0.74, 0.82, 0.86, 1.0]
        },
    ));

    // HP readout above the hint panel, aligned with the health bar.
    sections.push(section_bounded(
        layout.hint_panel.x,
        layout.hint_panel.y - (52.0 * s).clamp(38.0, 62.0),
        layout.hint_panel.w,
        24.0 * s,
        format!(
            "HP  {:.0} / {:.0}{}",
            state.health.max(0.0),
            state.max_health,
            state
                .money
                .map(|m| format!("     $ {m}"))
                .unwrap_or_default()
        ),
        scaled_font(14.0, s, 11.0, 17.0),
        [0.88, 0.92, 0.94, 1.0],
    ));

    if let Some(lines) = state.buy_lines {
        let row_h = (34.0 * s).clamp(26.0, 40.0);
        let panel_w = (720.0 * s).clamp(520.0, 860.0).min(screen_width * 0.92);
        let panel_h = row_h * (lines.len() as f32 + 2.5);
        let panel_x = (screen_width - panel_w) * 0.5;
        let panel_y = (screen_height - panel_h) * 0.42;
        let title_font = scaled_font(20.0, s, 16.0, 24.0);
        sections.push(section_bounded(
            panel_x + row_h * 0.6,
            panel_y + row_h * 0.4,
            panel_w - row_h * 1.2,
            title_font * 1.5,
            format!(
                "武器购买  ·  按 1-5 购买，B 关闭{}",
                state
                    .money
                    .map(|m| format!("     资金 ${m}"))
                    .unwrap_or_default()
            ),
            title_font,
            [1.0, 0.78, 0.36, 1.0],
        ));
        let row_font = scaled_font(16.0, s, 13.0, 19.0);
        for (i, line) in lines.iter().enumerate() {
            sections.push(section_bounded(
                panel_x + row_h * 0.6,
                panel_y + row_h * (1.8 + i as f32),
                panel_w - row_h * 1.2,
                row_h,
                line.clone(),
                row_font,
                [0.90, 0.94, 0.96, 1.0],
            ));
        }
    }

    if let Some(lines) = state.scoreboard {
        let row_h = (32.0 * s).clamp(24.0, 38.0);
        let panel_w = (640.0 * s).clamp(480.0, 780.0).min(screen_width * 0.9);
        let panel_h = row_h * (lines.len() as f32 + 2.2);
        let panel_x = (screen_width - panel_w) * 0.5;
        let panel_y = (screen_height - panel_h) * 0.35;
        let title_font = scaled_font(19.0, s, 15.0, 23.0);
        sections.push(section_bounded(
            panel_x + row_h * 0.6,
            panel_y + row_h * 0.35,
            panel_w - row_h * 1.2,
            title_font * 1.5,
            "记分板",
            title_font,
            [0.42, 0.86, 0.90, 1.0],
        ));
        let row_font = scaled_font(15.0, s, 12.5, 18.0);
        for (i, line) in lines.iter().enumerate() {
            sections.push(section_bounded(
                panel_x + row_h * 0.6,
                panel_y + row_h * (1.6 + i as f32),
                panel_w - row_h * 1.2,
                row_h,
                line.clone(),
                row_font,
                if line.contains("[我方]") {
                    [0.55, 0.75, 1.0, 1.0]
                } else {
                    [1.0, 0.62, 0.55, 1.0]
                },
            ));
        }
    }

    if let Some(line) = state.match_line {
        let font = scaled_font(16.0, s, 13.0, 19.0);
        let width = (560.0 * s).clamp(380.0, 660.0).min(screen_width * 0.8);
        sections.push(section_bounded(
            (screen_width - width) * 0.5,
            (10.0 * s).clamp(7.0, 13.0),
            width,
            font * 1.6,
            line,
            font,
            [1.0, 0.84, 0.44, 1.0],
        ));
    }

    if let Some(toast) = state.toast {
        // Mirror the banner rectangle drawn in `draw_hud`: centered, y=14%.
        let toast_w = (620.0 * s).clamp(400.0, 760.0).min(screen_width * 0.9);
        let toast_h = (54.0 * s).clamp(40.0, 66.0);
        let font = scaled_font(17.0, s, 13.0, 20.0);
        sections.push(section_bounded(
            (screen_width - toast_w) * 0.5 + (18.0 * s).clamp(12.0, 22.0),
            screen_height * 0.14 + (toast_h - font) * 0.42,
            toast_w - (36.0 * s).clamp(24.0, 44.0),
            toast_h,
            toast,
            font,
            [1.0, 0.86, 0.52, 1.0],
        ));
    }

    sections.push(section_bounded(
        layout.weapon_panel.x + pad,
        layout.weapon_panel.y + (9.0 * s).clamp(6.0, 11.0),
        layout.weapon_panel.w * 0.58,
        layout.weapon_panel.h * 0.42,
        state.weapon_name,
        scaled_font(15.0, s, 12.0, 18.0),
        [1.0, 0.70, 0.30, 1.0],
    ));
    sections.push(section_bounded(
        layout.weapon_panel.x + pad,
        layout.weapon_panel.y + layout.weapon_panel.h * 0.40,
        layout.weapon_panel.w - pad * 2.0,
        layout.weapon_panel.h * 0.42,
        if state.is_reloading {
            format!("装填中  {:.1}s", state.reload_timer.max(0.0))
        } else {
            format!("{:02}  /  {:02}", state.current_ammo, state.max_ammo)
        },
        scaled_font(27.0, s, 20.0, 32.0),
        if !state.is_reloading && state.current_ammo * 5 <= state.max_ammo {
            [1.0, 0.40, 0.30, 1.0]
        } else {
            text
        },
    ));

    if state.runtime.paused {
        let font = scaled_font(14.0, s, 11.0, 17.0);
        sections.push(section_bounded(
            layout.pause_badge.x + 12.0 * s,
            centered_text_y(layout.pause_badge, font),
            layout.pause_badge.w - 24.0 * s,
            layout.pause_badge.h,
            "Ⅱ  模拟暂停  ·  . 单步",
            font,
            [1.0, 0.88, 0.84, 1.0],
        ));
    }

    if !state.focus.prompt.is_empty() {
        let key_font = scaled_font(17.0, s, 13.0, 20.0);
        let key_size = (42.0 * s).clamp(31.0, 49.0);
        sections.push(section_bounded(
            layout.interaction_panel.x + 14.0 * s,
            layout.interaction_panel.y
                + (layout.interaction_panel.h - key_size) * 0.5
                + (key_size - key_font) * 0.36,
            key_size,
            key_size,
            "E",
            key_font,
            [0.10, 0.075, 0.035, 1.0],
        ));
        let text_x = layout.interaction_panel.x + key_size + 28.0 * s;
        sections.push(section_bounded(
            text_x,
            layout.interaction_panel.y + (8.0 * s).clamp(6.0, 10.0),
            layout.interaction_panel.right() - text_x - 14.0 * s,
            layout.interaction_panel.h * 0.46,
            format!("{}   ·   {:.1}m", state.focus.title, state.focus.distance),
            scaled_font(17.0, s, 13.0, 20.0),
            [1.0, 0.78, 0.39, 1.0],
        ));
        sections.push(section_bounded(
            text_x,
            layout.interaction_panel.y + layout.interaction_panel.h * 0.55,
            layout.interaction_panel.right() - text_x - 14.0 * s,
            layout.interaction_panel.h * 0.35,
            &state.focus.prompt,
            scaled_font(13.0, s, 10.5, 16.0),
            text,
        ));
    }
    sections
}

pub(super) fn build_help_text_sections(layout: &HudLayout) -> Vec<OwnedSection> {
    let mut sections = Vec::with_capacity(10);
    let s = layout.scale;
    let text = [0.91, 0.95, 0.97, 1.0];
    let muted = [0.60, 0.68, 0.73, 1.0];
    let header_pad = (28.0 * s).clamp(18.0, 34.0);
    sections.push(section_bounded(
        layout.help_header.x + header_pad,
        layout.help_header.y + (13.0 * s).clamp(9.0, 16.0),
        layout.help_header.w * 0.7,
        layout.help_header.h,
        "操作指南  /  CONTROLS",
        scaled_font(26.0, s, 20.0, 31.0),
        text,
    ));
    sections.push(section_bounded(
        layout.help_header.x + header_pad,
        layout.help_header.y + layout.help_header.h * 0.60,
        layout.help_header.w * 0.7,
        layout.help_header.h * 0.32,
        "按住 F 查看 · 松开立即返回实验",
        scaled_font(13.0, s, 10.5, 16.0),
        muted,
    ));
    add_help_card_text(
        &mut sections,
        layout.help_cards[0],
        s,
        "01  移动 / 观察",
        "WASD\n移动\n\nSHIFT\n奔跑 / 加速飞行\n\nSPACE / CTRL\n跳跃、上升 / 下降\n\nV\nFPS / 上帝模式",
        [0.36, 0.84, 0.88, 1.0],
    );
    add_help_card_text(
        &mut sections,
        layout.help_cards[1],
        s,
        "02  交互 / 沙盒",
        "左键 / 右键 / R\n射击 / 瞄准 / 装填\n\nB  购买武器（1-5 选择）\n\nE / 中键 / 滚轮\n抓取、放下 / 调整距离\n\nT / X / DELETE\n投掷、冻结 / 删除\n\nG / N / H\n箱子 / 弹力球 / 重箱",
        [1.0, 0.70, 0.32, 1.0],
    );
    add_help_card_text(
        &mut sections,
        layout.help_cards[2],
        s,
        "03  模拟 / 诊断",
        "P / . / [ ]\n暂停 / 单步 / 调整倍速\n\nF3\n碰撞体线框\n\nF4 / F5\n速度向量 / 接触点\n\nESC\n打开设置菜单",
        [0.76, 0.68, 1.0, 1.0],
    );
    sections.push(section_bounded(
        layout.help_footer.x + header_pad,
        layout.help_footer.y + layout.help_footer.h * 0.29,
        layout.help_footer.w - header_pad * 2.0,
        layout.help_footer.h,
        "准星变为橙色时，当前物体可交互 · HUD 会持续显示模拟速度与诊断图层",
        scaled_font(13.0, s, 10.0, 15.0),
        muted,
    ));
    sections
}

fn add_help_card_text(
    sections: &mut Vec<OwnedSection>,
    rect: UiRect,
    scale: f32,
    title: &str,
    body: &str,
    accent: [f32; 4],
) {
    let pad = (20.0 * scale).clamp(13.0, 24.0);
    let title_font = scaled_font(18.0, scale, 14.0, 22.0);
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad,
        rect.w - pad * 2.0,
        title_font * 1.5,
        title,
        title_font,
        accent,
    ));
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad + title_font * 2.2,
        rect.w - pad * 2.0,
        rect.h - pad * 2.0 - title_font * 2.0,
        body,
        scaled_font(14.5, scale, 11.0, 17.0),
        [0.84, 0.89, 0.92, 1.0],
    ));
}

pub(super) fn build_help_rectangles(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    layout: &HudLayout,
) {
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        },
        [0.005, 0.009, 0.014, 0.72],
    );
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: layout.help_shell.x + 8.0 * layout.scale,
            y: layout.help_shell.y + 10.0 * layout.scale,
            ..layout.help_shell
        },
        [0.0, 0.0, 0.0, 0.30],
    );
    add_rect(
        vertices,
        width,
        height,
        layout.help_shell,
        [0.030, 0.040, 0.051, 0.98],
    );
    add_outline(
        vertices,
        width,
        height,
        layout.help_shell,
        [0.22, 0.33, 0.41, 0.94],
    );
    add_rect(
        vertices,
        width,
        height,
        layout.help_header,
        [0.048, 0.063, 0.078, 0.98],
    );
    add_rect(
        vertices,
        width,
        height,
        layout.help_footer,
        [0.024, 0.032, 0.041, 0.98],
    );
    for (card, color) in layout.help_cards.iter().zip([
        [0.16, 0.71, 0.77, 0.95],
        [0.94, 0.58, 0.14, 0.95],
        [0.56, 0.48, 0.94, 0.95],
    ]) {
        add_card(vertices, width, height, *card, color);
    }
}
