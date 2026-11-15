//! 主题与字体：颜色常量、卡片配色、字体/字号/视觉初始化。
//! UI 代码只依赖这里导出的常量，配色调整集中在一处。

use eframe::egui::{self, Color32, FontId, Rounding, Stroke};

pub const ACCENT: Color32 = Color32::from_rgb(88, 122, 250);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(230, 237, 255);
pub const ERROR_RED: Color32 = Color32::from_rgb(232, 88, 88);

pub const TEXT_DARK: Color32 = Color32::from_rgb(52, 56, 68);
pub const TEXT_SOFT: Color32 = Color32::from_rgb(118, 125, 140);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(168, 175, 190);

pub const BG_SIDEBAR: Color32 = Color32::from_rgb(247, 248, 252);
pub const BG_EDITOR: Color32 = Color32::from_rgb(255, 255, 255);

/// 六张便签的柔和底色。
pub const NOTE_COLORS: [(u8, u8, u8); 6] = [
    (255, 243, 176), // 鹅黄
    (255, 219, 219), // 粉
    (216, 231, 255), // 蓝
    (213, 245, 220), // 绿
    (235, 223, 255), // 紫
    (255, 230, 199), // 杏橙
];

pub fn color_for(i: usize) -> Color32 {
    let (r, g, b) = NOTE_COLORS[i % NOTE_COLORS.len()];
    Color32::from_rgb(r, g, b)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        lerp(a.r() as f32, b.r() as f32, t) as u8,
        lerp(a.g() as f32, b.g() as f32, t) as u8,
        lerp(a.b() as f32, b.b() as f32, t) as u8,
        lerp(a.a() as f32, b.a() as f32, t) as u8,
    )
}

/// 界面上出现的全部静态文案，以及数字/字母/标点。
/// 启动预热时连同所有便签正文一起排版，让字体图集一次扩容到位。
const WARMUP_STATIC_TEXT: &str = "Memo便签新建搜索无标题写点什么创建于最近编辑字张还没有便签没有匹配的便签选择或新建一张便签确定删除这张便签吗此操作无法撤销删除取消复制剪切粘贴全选就绪自动保存已开启已自动保存正在输入保存失败从旧版迁移快捷键刚刚分钟前小时前昨天天前个月前鹅黄粉蓝绿紫杏橙无标题确定取消知道了保存状态数据库写入失败颜色设置新建便签搜索便签内容字数第一二三四五六七八九十零0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ，。！？、：；“”‘’（）【】《》—…·.-+/ :()[]{}#@%&*!?\n\t";

/// 界面实际用到的字号（painter.layout 与 TextStyle 两种路径都要覆盖）。
const WARMUP_SIZES: [f32; 8] = [11.0, 12.0, 12.5, 14.5, 15.5, 16.0, 20.5, 22.0];

/// 启动期字体预热。
///
/// egui 的字体图集初始高度只有 32px，且每帧在填充率超过 0.8 时会重建整个
/// Fonts（清空字形缓存）。内嵌中文字体 + 启动时恢复的便签正文会连续触发
/// 数次图集扩容/重建，每次纹理重新上传即表现为「启动后闪几下，之后稳定」。
///
/// 这里把界面所有文案与全部便签文本去重后，按所有字号一次性排版，
/// 让图集在用户看到界面之前就扩容到位；配合启动预热帧（只画纯色背景）
/// 把重建过程完全挡住。必须在 `App::update` 里调用：`Context::fonts()`
/// 在第一次 `run()` 之前不可用。
pub fn warmup_fonts(ctx: &egui::Context, notes: &crate::model::Notes) {
    let mut all = String::with_capacity(WARMUP_STATIC_TEXT.len() + 512);
    all.push_str(WARMUP_STATIC_TEXT);
    for note in &notes.notes {
        all.push_str(&note.title);
        all.push('\n');
        all.push_str(&note.content);
    }

    let mut chars: Vec<char> = all.chars().collect();
    chars.sort_unstable();
    chars.dedup();
    let text: String = chars.into_iter().collect();

    ctx.fonts(|fonts| {
        for size in WARMUP_SIZES {
            let _ = fonts.layout_no_wrap(text.clone(), FontId::proportional(size), Color32::BLACK);
        }
    });
}

const EMBEDDED_FONT: &[u8] = include_bytes!("../assets/font.otf");

/// 内嵌 Noto Sans SC 子集，中英文统一渲染，杜绝缺字。
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".to_owned(),
        egui::FontData::from_owned(EMBEDDED_FONT.to_vec()),
    );
    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&fam) {
            list.insert(0, "cjk".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

pub fn setup_text_styles(ctx: &egui::Context) {
    use egui::{FontFamily, TextStyle};
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(22.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.5, FontFamily::Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(14.5, FontFamily::Monospace),
        ),
        (
            TextStyle::Button,
            FontId::new(14.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
    ]
    .into();
    ctx.set_style(style);
}

/// 强制浅色主题，不跟随系统深色模式；统一圆角、选区、按钮与文字颜色。
pub fn setup_visuals(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);

    let mut v = egui::Visuals::light();
    v.panel_fill = BG_SIDEBAR;
    v.window_fill = BG_EDITOR;
    v.extreme_bg_color = BG_EDITOR;
    v.faint_bg_color = Color32::from_rgb(242, 244, 249);
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.widgets.inactive.bg_fill = Color32::from_rgb(239, 241, 248);
    v.widgets.inactive.rounding = Rounding::same(9.0);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_DARK);
    v.widgets.hovered.bg_fill = ACCENT_SOFT;
    v.widgets.hovered.rounding = Rounding::same(9.0);
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.rounding = Rounding::same(9.0);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.override_text_color = Some(TEXT_DARK);
    v.text_cursor.stroke = Stroke::new(2.0, ACCENT);
    v.window_rounding = Rounding::same(12.0);
    ctx.set_visuals(v);
}
