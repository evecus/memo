//! 主题与字体：颜色常量、便签配色、字体注册、强制浅色主题。
//! UI 代码只依赖这里导出的常量，配色调整集中在一处。

use std::borrow::Cow;

use gpui::{App, Rgba};
use gpui_component::{Theme, ThemeMode};

/// 内嵌 Noto Sans SC 子集，中英文统一渲染，杜绝缺字。
const EMBEDDED_FONT: &[u8] = include_bytes!("../assets/font.otf");
/// `assets/font.otf` 的 family name（从字体 name 表读出）。
pub const FONT_FAMILY: &str = "Noto Sans CJK SC";

/// 编辑区正文字号。
pub const EDITOR_FONT_SIZE: f32 = 17.0;

const fn rgb8(r: u8, g: u8, b: u8) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

const fn black_alpha(a: u8) -> Rgba {
    Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: a as f32 / 255.0,
    }
}

pub const ACCENT: Rgba = rgb8(88, 122, 250);
pub const ACCENT_HOVER: Rgba = rgb8(72, 104, 232);
pub const ACCENT_SOFT: Rgba = rgb8(230, 237, 255);
pub const ERROR_RED: Rgba = rgb8(232, 88, 88);
pub const ERROR_SOFT: Rgba = rgb8(255, 238, 238);

pub const TEXT_DARK: Rgba = rgb8(52, 56, 68);
pub const TEXT_SOFT: Rgba = rgb8(118, 125, 140);
pub const TEXT_FAINT: Rgba = rgb8(168, 175, 190);

pub const BG_SIDEBAR: Rgba = rgb8(247, 248, 252);
pub const BG_EDITOR: Rgba = rgb8(255, 255, 255);
pub const BG_CHIP: Rgba = rgb8(243, 245, 250);
pub const BG_CHIP_HOVER: Rgba = rgb8(232, 235, 243);
pub const DIVIDER: Rgba = rgb8(236, 238, 244);
pub const OVERLAY: Rgba = black_alpha(60);
pub const CARD_BORDER: Rgba = black_alpha(12);

/// 六张便签的柔和底色。
pub const NOTE_COLORS: [Rgba; 6] = [
    rgb8(255, 243, 176), // 鹅黄
    rgb8(255, 219, 219), // 粉
    rgb8(216, 231, 255), // 蓝
    rgb8(213, 245, 220), // 绿
    rgb8(235, 223, 255), // 紫
    rgb8(255, 230, 199), // 杏橙
];

pub fn color_for(i: usize) -> Rgba {
    NOTE_COLORS[i % NOTE_COLORS.len()]
}

/// 卡片未悬停时的底色：原色压暗到 94%，悬停/选中时恢复原色。
pub fn dimmed(c: Rgba) -> Rgba {
    Rgba {
        r: c.r * 0.94,
        g: c.g * 0.94,
        b: c.b * 0.94,
        a: c.a,
    }
}

/// 注册内嵌字体并强制浅色主题（不跟随系统深色模式）。
pub fn init(cx: &mut App) {
    if let Err(e) = cx
        .text_system()
        .add_fonts(vec![Cow::Borrowed(EMBEDDED_FONT)])
    {
        eprintln!("加载内嵌字体失败，将回退到系统字体：{e}");
    }
    Theme::change(ThemeMode::Light, None, cx);
}
