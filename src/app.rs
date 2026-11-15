//! 应用状态与生命周期：数据库句柄、内存缓存、自动保存调度、快捷键与窗口布局编排。
//! 具体绘制交给 `ui::sidebar` / `ui::editor` / `ui::dialogs`。

use std::time::{Duration, Instant};

use eframe::egui;

use crate::db::{Db, DbResult};
use crate::model::{Note, Notes};
use crate::theme;
use crate::util::now_secs;

/// 应用图标（内嵌，同时用于侧栏 Logo 与窗口/任务栏图标）。
pub const APP_ICON: &[u8] = include_bytes!("../assets/app.ico");

const SAVE_DEBOUNCE: Duration = Duration::from_millis(700);
const SAVE_HEARTBEAT: Duration = Duration::from_secs(5);
const REPAINT_INTERVAL: Duration = Duration::from_secs(1);
const WARMUP_MAX_FRAMES: u8 = 12;
const WARMUP_STABLE_FRAMES: u8 = 2;

pub struct NoteApp {
    pub db: Db,
    pub notes: Notes,
    pub current: Option<u64>,
    pub dirty: bool,
    pub dirty_since: Option<Instant>,
    pub last_save: Instant,
    pub search: String,
    pub confirm_delete: bool,
    pub context_menu_note: Option<u64>,
    pub status: String,
    pub status_ok: bool,
    /// 一次性提示（例如旧数据迁移完成），显示在编辑区顶部，可手动关闭。
    pub notice: Option<String>,
    #[allow(dead_code)]
    pub logo_texture: Option<egui::TextureHandle>,
    pub focus_search: bool,

    // 启动预热：把字体图集重建挡在纯色背景帧里
    warmup_done: bool,
    warmup_frames: u8,
    warmup_last_fill: f32,
    warmup_stable: u8,
}

impl NoteApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        theme::setup_fonts(&cc.egui_ctx);
        theme::setup_text_styles(&cc.egui_ctx);
        theme::setup_visuals(&cc.egui_ctx);

        // 数据库打开失败直接向上传播，不静默降级到 JSON 文件。
        let db = Db::open()?;
        let notes = db.load()?;
        let current = notes.last_open.filter(|id| notes.get(*id).is_some());

        let notice = db
            .migrated_from_legacy
            .map(|n| format!("已从旧版 notes.json 迁移 {n} 张便签，数据现保存在 memo.redb"));

        Ok(Self {
            db,
            notes,
            current,
            dirty: false,
            dirty_since: None,
            last_save: Instant::now(),
            search: String::new(),
            confirm_delete: false,
            context_menu_note: None,
            status: "就绪 · 自动保存已开启".to_owned(),
            status_ok: true,
            notice,
            logo_texture: load_logo(&cc.egui_ctx),
            focus_search: false,
            warmup_done: false,
            warmup_frames: 0,
            warmup_last_fill: 0.0,
            warmup_stable: 0,
        })
    }

    // ---------- 状态变更 ----------

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        if self.dirty_since.is_none() {
            self.dirty_since = Some(Instant::now());
        }
        if let Some(id) = self.current {
            if let Some(note) = self.notes.get_mut(id) {
                note.updated = now_secs();
            }
        }
        self.status = "正在输入…".to_owned();
        self.status_ok = true;
    }

    fn report(&mut self, result: DbResult<()>, ok_msg: &str) {
        match result {
            Ok(()) => {
                self.status = ok_msg.to_owned();
                self.status_ok = true;
            }
            Err(e) => {
                self.status = format!("数据库写入失败：{e}");
                self.status_ok = false;
            }
        }
    }

    /// 一次自动保存只写「当前便签 + next_id + last_open」一个事务。
    pub(crate) fn save(&mut self) {
        let note = self.current.and_then(|id| self.notes.get(id)).cloned();
        let result = self
            .db
            .save_state(note.as_ref(), self.current, self.notes.next_id);
        self.report(result, "已自动保存");
        self.dirty = false;
        self.dirty_since = None;
        self.last_save = Instant::now();
    }

    pub(crate) fn new_note(&mut self) {
        let id = self.notes.next_id;
        self.notes.next_id += 1;
        let color = (id as usize) % theme::NOTE_COLORS.len();
        self.notes.notes.push(Note::new(id, color));
        self.current = Some(id);
        self.confirm_delete = false;
        self.save();
    }

    pub(crate) fn delete_current(&mut self) {
        let Some(id) = self.context_menu_note.or(self.current) else {
            return;
        };
        let deleted = self.db.delete(id);
        self.notes.notes.retain(|n| n.id != id);
        if self.current == Some(id) {
            self.current = None;
        }
        self.context_menu_note = None;
        self.confirm_delete = false;

        let state = self.db.save_state(None, self.current, self.notes.next_id);
        self.report(deleted.and(state), "已删除");
        self.dirty = false;
        self.dirty_since = None;
        self.last_save = Instant::now();
    }

    /// 切换便签底色：改内存并立即落库。
    pub(crate) fn set_color(&mut self, idx: usize) {
        if let Some(id) = self.current {
            if let Some(note) = self.notes.get_mut(id) {
                note.color = idx;
                note.updated = now_secs();
            }
        }
        self.save();
    }

    // ---------- 启动预热 ----------

    fn warmup_step(&mut self, ctx: &egui::Context) {
        // 只画纯色背景：此时不渲染任何文字或纹理，用户看不到图集重建。
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::BG_EDITOR))
            .show(ctx, |_| {});

        // 本帧 begin_pass 之后的填充率。若比上一帧末尾明显变小，
        // 说明字体对象被重建（字形缓存清空），需要继续预热。
        let fill_before = ctx.fonts(|f| f.font_atlas_fill_ratio());
        theme::warmup_fonts(ctx, &self.notes);
        let fill_after = ctx.fonts(|f| f.font_atlas_fill_ratio());

        let rebuilt = fill_before + 0.02 < self.warmup_last_fill;
        self.warmup_stable = if rebuilt {
            0
        } else {
            self.warmup_stable.saturating_add(1)
        };
        self.warmup_last_fill = fill_after;
        self.warmup_frames = self.warmup_frames.saturating_add(1);

        if self.warmup_stable >= WARMUP_STABLE_FRAMES || self.warmup_frames >= WARMUP_MAX_FRAMES {
            self.warmup_done = true;
        } else {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for NoteApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.warmup_done {
            self.warmup_step(ctx);
            return;
        }

        // 自动保存：700ms 防抖 / 5s 兜底
        if self.dirty {
            let debounced = self
                .dirty_since
                .is_some_and(|t| t.elapsed() >= SAVE_DEBOUNCE);
            if debounced || self.last_save.elapsed() >= SAVE_HEARTBEAT {
                self.save();
            }
        }

        // 快捷键
        if ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.command) {
            self.new_note();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.command) {
            self.save();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.command) {
            self.focus_search = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.confirm_delete = false;
            self.context_menu_note = None;
        }

        let win_w = ctx.screen_rect().width();
        let sidebar_w = (win_w * 0.24).clamp(240.0, 340.0);
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(sidebar_w)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_SIDEBAR)
                    .inner_margin(egui::Margin::same(14.0)),
            )
            .show(ctx, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_EDITOR)
                    .inner_margin(egui::Margin::same(20.0)),
            )
            .show(ctx, |ui| self.editor(ui));

        self.delete_dialog(ctx);

        // 只需刷新「N 分钟前」这类相对时间，1 秒一次足够。
        // 原版 300ms 无条件重绘会放大启动期的纹理重建闪烁。
        ctx.request_repaint_after(REPAINT_INTERVAL);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}

fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(APP_ICON).ok()?.into_rgba8();
    let (width, height) = (image.width(), image.height());
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], image.as_raw());
    Some(ctx.load_texture("app_logo", color_image, egui::TextureOptions::LINEAR))
}

/// 供窗口图标使用，避免 main.rs 重复内嵌同一份字节。
pub fn load_app_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(APP_ICON).ok()?.into_rgba8();
    let (width, height) = (image.width(), image.height());
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
