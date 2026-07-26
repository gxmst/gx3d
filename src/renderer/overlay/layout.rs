use crate::game::UiRect;

#[derive(Debug, Clone, Copy)]
pub(super) struct HudLayout {
    pub(super) scale: f32,
    pub(super) mode_panel: UiRect,
    pub(super) telemetry_panel: UiRect,
    pub(super) debug_panel: UiRect,
    pub(super) hint_panel: UiRect,
    pub(super) weapon_panel: UiRect,
    pub(super) interaction_panel: UiRect,
    pub(super) pause_badge: UiRect,
    pub(super) help_shell: UiRect,
    pub(super) help_header: UiRect,
    pub(super) help_cards: [UiRect; 3],
    pub(super) help_footer: UiRect,
}

impl HudLayout {
    pub(super) fn new(width: f32, height: f32) -> Self {
        let width = width.max(1.0);
        let height = height.max(1.0);
        let scale = (height / 1080.0).min(width / 1600.0).clamp(0.68, 1.18);
        let margin = (28.0 * scale)
            .clamp(18.0, 36.0)
            .min(width * 0.04)
            .min(height * 0.04);
        let mode_w = (300.0 * scale).clamp(230.0, 350.0);
        let top_h = (82.0 * scale).clamp(58.0, 96.0);
        let telemetry_w = (360.0 * scale).clamp(275.0, 420.0);
        let mode_panel = UiRect {
            x: margin,
            y: margin,
            w: mode_w,
            h: top_h,
        };
        let telemetry_panel = UiRect {
            x: width - margin - telemetry_w,
            y: margin,
            w: telemetry_w,
            h: top_h,
        };
        let debug_panel = UiRect {
            x: margin,
            y: mode_panel.bottom() + (10.0 * scale).max(7.0),
            w: (438.0 * scale).clamp(330.0, 510.0),
            h: (40.0 * scale).clamp(30.0, 47.0),
        };
        let weapon_w = (350.0 * scale).clamp(270.0, 410.0);
        let weapon_h = (118.0 * scale).clamp(84.0, 138.0);
        let weapon_panel = UiRect {
            x: width - margin - weapon_w,
            y: height - margin - weapon_h,
            w: weapon_w,
            h: weapon_h,
        };
        let hint_w = (420.0 * scale).clamp(315.0, 480.0);
        let hint_h = (70.0 * scale).clamp(50.0, 82.0);
        let hint_panel = UiRect {
            x: margin,
            y: height - margin - hint_h,
            w: hint_w,
            h: hint_h,
        };
        let interaction_w = (590.0 * scale)
            .clamp(370.0, 690.0)
            .min(width - margin * 2.0);
        let interaction_h = (82.0 * scale).clamp(58.0, 96.0);
        let interaction_panel = UiRect {
            x: (width - interaction_w) * 0.5,
            y: (height * 0.5 + 56.0 * scale).min(height - margin - interaction_h - weapon_h * 0.18),
            w: interaction_w,
            h: interaction_h,
        };
        let pause_w = (210.0 * scale).clamp(160.0, 245.0);
        let pause_badge = UiRect {
            x: (width - pause_w) * 0.5,
            y: margin,
            w: pause_w,
            h: (40.0 * scale).clamp(30.0, 47.0),
        };

        let help_margin = (40.0 * scale)
            .clamp(20.0, 48.0)
            .min(width * 0.05)
            .min(height * 0.05);
        let help_w = (1180.0 * scale).min(width - help_margin * 2.0).max(1.0);
        let help_h = (720.0 * scale).min(height - help_margin * 2.0).max(1.0);
        let help_shell = UiRect {
            x: (width - help_w) * 0.5,
            y: (height - help_h) * 0.5,
            w: help_w,
            h: help_h,
        };
        let help_header_h = (82.0 * scale).clamp(56.0, 96.0);
        let help_footer_h = (48.0 * scale).clamp(34.0, 56.0);
        let help_header = UiRect {
            x: help_shell.x,
            y: help_shell.y,
            w: help_shell.w,
            h: help_header_h,
        };
        let help_footer = UiRect {
            x: help_shell.x,
            y: help_shell.bottom() - help_footer_h,
            w: help_shell.w,
            h: help_footer_h,
        };
        let card_gap = (14.0 * scale).clamp(9.0, 17.0);
        let card_pad = (20.0 * scale).clamp(13.0, 24.0);
        let cards_y = help_header.bottom() + card_pad;
        let cards_h = (help_footer.y - card_pad - cards_y).max(1.0);
        let cards_w = ((help_shell.w - card_pad * 2.0 - card_gap * 2.0) / 3.0).max(1.0);
        let help_cards = std::array::from_fn(|index| UiRect {
            x: help_shell.x + card_pad + index as f32 * (cards_w + card_gap),
            y: cards_y,
            w: cards_w,
            h: cards_h,
        });

        Self {
            scale,
            mode_panel,
            telemetry_panel,
            debug_panel,
            hint_panel,
            weapon_panel,
            interaction_panel,
            pause_badge,
            help_shell,
            help_header,
            help_cards,
            help_footer,
        }
    }
}

pub(super) fn debug_badge_rects(layout: &HudLayout) -> [UiRect; 4] {
    let pad = (6.0 * layout.scale).clamp(4.0, 7.0);
    let gap = (6.0 * layout.scale).clamp(4.0, 7.0);
    let badge_w = (layout.debug_panel.w - pad * 2.0 - gap * 3.0) / 4.0;
    std::array::from_fn(|index| UiRect {
        x: layout.debug_panel.x + pad + index as f32 * (badge_w + gap),
        y: layout.debug_panel.y + pad,
        w: badge_w,
        h: layout.debug_panel.h - pad * 2.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_inside(rect: UiRect, width: f32, height: f32) {
        assert!(rect.x >= -0.01, "left edge escaped: {rect:?}");
        assert!(rect.y >= -0.01, "top edge escaped: {rect:?}");
        assert!(rect.right() <= width + 0.01, "right edge escaped: {rect:?}");
        assert!(
            rect.bottom() <= height + 0.01,
            "bottom edge escaped: {rect:?}"
        );
    }

    fn overlaps(a: UiRect, b: UiRect) -> bool {
        a.x < b.right() && a.right() > b.x && a.y < b.bottom() && a.bottom() > b.y
    }

    #[test]
    fn hud_layout_stays_inside_supported_viewports() {
        for (width, height) in [
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3440.0, 1440.0),
        ] {
            let layout = HudLayout::new(width, height);
            for rect in [
                layout.mode_panel,
                layout.telemetry_panel,
                layout.debug_panel,
                layout.hint_panel,
                layout.weapon_panel,
                layout.interaction_panel,
                layout.pause_badge,
                layout.help_shell,
                layout.help_header,
                layout.help_footer,
                layout.help_cards[0],
                layout.help_cards[1],
                layout.help_cards[2],
            ] {
                assert_inside(rect, width, height);
            }
            assert!(!overlaps(layout.mode_panel, layout.telemetry_panel));
            assert!(!overlaps(layout.hint_panel, layout.weapon_panel));
        }
    }
}
