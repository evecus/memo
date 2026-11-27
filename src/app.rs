//! 应用状态与生命周期：数据库句柄、内存缓存、输入框实体、自动保存调度、快捷键与布局编排。
//! 具体绘制交给 `ui::sidebar` / `ui::editor` / `ui::dialogs`。

use std::time::{Duration, Instant};

use gpui::{
    actions, div, prelude::*, App, Context, Entity, FocusHandle, Focusable, KeyBinding, Pixels,
    Point, Subscription, Task, Window,
};
use gpui_component::input::{InputEvent, InputState};

use crate::db::{Db, DbResult};
use crate::model::{Note, Notes};
use crate::theme::{self, BG_EDITOR, FONT_FAMILY, TEXT_DARK};
use crate::util::now_secs;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(700);
const SAVE_HEARTBEAT: Duration = Duration::from_secs(5);
/// 自动保存检查间隔；每 4 次（约 1 秒）刷新一次「N 分钟前」这类相对时间。
const TICK: Duration = Duration::from_millis(250);
const REPAINT_EVERY_TICKS: u32 = 4;

actions!(memo, [NewNote, SaveNow, FocusSearch, Dismiss]);

/// 全局快捷键。`secondary` 在 macOS 上是 ⌘，其它平台是 Ctrl。
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-n", NewNote, None),
        KeyBinding::new("secondary-s", SaveNow, None),
        KeyBinding::new("secondary-f", FocusSearch, None),
        KeyBinding::new("escape", Dismiss, None),
    ]);
}

pub struct NoteApp {
    pub db: Db,
    pub notes: Notes,
    pub current: Option<u64>,
    pub dirty: bool,
    pub dirty_since: Option<Instant>,
    pub last_save: Instant,
    pub status: String,
    pub status_ok: bool,
    /// 一次性提示（例如旧数据迁移完成），显示在编辑区顶部，可手动关闭。
    pub notice: Option<String>,
    /// 等待确认删除的便签 id。
    pub confirm_delete: Option<u64>,
    /// 卡片右键菜单：(便签 id, 鼠标位置)。
    pub card_menu: Option<(u64, Point<Pixels>)>,

    pub search_input: Entity<InputState>,
    pub title_input: Entity<InputState>,
    pub content_input: Entity<InputState>,

    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
    _ticker: Task<()>,
}

impl NoteApp {
    pub fn new(db: Db, notes: Notes, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let current = notes.last_open.filter(|id| notes.get(*id).is_some());
        let notice = db
            .migrated_from_legacy
            .map(|n| format!("已从旧版 notes.json 迁移 {n} 张便签，数据现保存在 memo.redb"));

        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索便签…"));
        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("无标题"));
        // 多行输入框自带滚动视口：长内容可以滚到最后一行，不会把布局撑出窗口。
        let content_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line()
                .placeholder("写点什么…")
        });

        let subscriptions = vec![
            cx.subscribe_in(&search_input, window, |_, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change { .. }) {
                    cx.notify();
                }
            }),
            cx.subscribe_in(&title_input, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change { .. }) {
                    this.sync_title(cx);
                }
            }),
            cx.subscribe_in(&content_input, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change { .. }) {
                    this.sync_content(cx);
                }
            }),
            // 退出前最后保存一次。
            cx.on_app_quit(|this, _cx| {
                this.save();
                async {}
            }),
        ];

        // 视图被释放（窗口关闭）时兜底保存。
        cx.on_release(|this, _cx| this.save()).detach();

        // 后台定时器：驱动防抖自动保存，并每秒重绘一次以刷新相对时间。
        let ticker = cx.spawn(async move |this, cx| {
            let mut ticks: u32 = 0;
            loop {
                cx.background_executor().timer(TICK).await;
                ticks = ticks.wrapping_add(1);
                let alive = this.update(cx, |this, cx| {
                    if this.autosave_due() {
                        this.save();
                        cx.notify();
                    } else if ticks % REPAINT_EVERY_TICKS == 0 {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let mut app = Self {
            db,
            notes,
            current,
            dirty: false,
            dirty_since: None,
            last_save: Instant::now(),
            status: "就绪 · 自动保存已开启".to_owned(),
            status_ok: true,
            notice,
            confirm_delete: None,
            card_menu: None,
            search_input,
            title_input,
            content_input,
            focus_handle,
            _subscriptions: subscriptions,
            _ticker: ticker,
        };
        app.load_inputs(window, cx);
        app
    }

    // ---------- 输入框 <-> 模型同步 ----------

    /// 把当前便签的标题/正文写入两个输入框（切换、新建、删除后调用）。
    pub(crate) fn load_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (title, content) = self
            .current
            .and_then(|id| self.notes.get(id))
            .map(|n| (n.title.clone(), n.content.clone()))
            .unwrap_or_default();
        self.title_input
            .update(cx, |s, cx| s.set_value(title, window, cx));
        self.content_input
            .update(cx, |s, cx| s.set_value(content, window, cx));
    }

    /// 输入框内容变化时回写模型。与模型值比较后再标脏：
    /// `set_value` 触发的 Change 事件不会被误判为用户编辑。
    fn sync_title(&mut self, cx: &mut Context<Self>) {
        let value = self.title_input.read(cx).value().to_string();
        if self.update_current(|n| {
            if n.title == value {
                false
            } else {
                n.title = value;
                true
            }
        }) {
            self.mark_dirty();
            cx.notify();
        }
    }

    fn sync_content(&mut self, cx: &mut Context<Self>) {
        let value = self.content_input.read(cx).value().to_string();
        if self.update_current(|n| {
            if n.content == value {
                false
            } else {
                n.content = value;
                true
            }
        }) {
            self.mark_dirty();
            cx.notify();
        }
    }

    fn update_current(&mut self, f: impl FnOnce(&mut Note) -> bool) -> bool {
        self.current
            .and_then(|id| self.notes.get_mut(id))
            .is_some_and(f)
    }

    // ---------- 状态变更 ----------

    fn mark_dirty(&mut self) {
        self.dirty = true;
        if self.dirty_since.is_none() {
            self.dirty_since = Some(Instant::now());
        }
        self.update_current(|n| {
            n.updated = now_secs();
            true
        });
        self.status = "正在输入…".to_owned();
        self.status_ok = true;
    }

    /// 700ms 防抖 / 5s 兜底。
    fn autosave_due(&self) -> bool {
        self.dirty
            && (self
                .dirty_since
                .is_some_and(|t| t.elapsed() >= SAVE_DEBOUNCE)
                || self.last_save.elapsed() >= SAVE_HEARTBEAT)
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

    pub(crate) fn new_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 先把正在编辑的便签落库，再切换。
        if self.dirty {
            self.save();
        }
        let id = self.notes.next_id;
        self.notes.next_id += 1;
        let color = (id as usize) % theme::NOTE_COLORS.len();
        self.notes.notes.push(Note::new(id, color));
        self.current = Some(id);
        self.confirm_delete = None;
        self.card_menu = None;
        self.save();
        self.load_inputs(window, cx);
        let handle = self.title_input.read(cx).focus_handle(cx);
        window.focus(&handle);
        cx.notify();
    }

    pub(crate) fn select(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.current == Some(id) {
            return;
        }
        if self.dirty {
            self.save();
        }
        self.current = Some(id);
        self.confirm_delete = None;
        self.card_menu = None;
        // 切换后立即记录，关闭时不编辑也能恢复到这张便签。
        self.save();
        self.load_inputs(window, cx);
        cx.notify();
    }

    pub(crate) fn request_delete(&mut self, id: u64, cx: &mut Context<Self>) {
        self.card_menu = None;
        self.confirm_delete = Some(id);
        cx.notify();
    }

    pub(crate) fn delete_confirmed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete.take() else {
            return;
        };
        let deleted = self.db.delete(id);
        self.notes.notes.retain(|n| n.id != id);
        let was_current = self.current == Some(id);
        if was_current {
            self.current = None;
        }

        let state = self.db.save_state(None, self.current, self.notes.next_id);
        self.report(deleted.and(state), "已删除");
        if was_current {
            self.dirty = false;
            self.dirty_since = None;
            self.load_inputs(window, cx);
        }
        self.last_save = Instant::now();
        cx.notify();
    }

    /// 切换便签底色：改内存并立即落库。
    pub(crate) fn set_color(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.update_current(|n| {
            n.color = idx;
            n.updated = now_secs();
            true
        });
        self.save();
        cx.notify();
    }

    // ---------- 动作 ----------

    fn on_new_note(&mut self, _: &NewNote, window: &mut Window, cx: &mut Context<Self>) {
        self.new_note(window, cx);
    }

    fn on_save_now(&mut self, _: &SaveNow, _: &mut Window, cx: &mut Context<Self>) {
        self.save();
        self.status = "已保存".to_owned();
        cx.notify();
    }

    fn on_focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.search_input.read(cx).focus_handle(cx);
        window.focus(&handle);
    }

    fn on_dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.confirm_delete.is_some() || self.card_menu.is_some() {
            self.confirm_delete = None;
            self.card_menu = None;
            cx.notify();
        }
    }
}

impl Focusable for NoteApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NoteApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let win_w = f32::from(window.viewport_size().width);
        let sidebar_w = (win_w * 0.24).clamp(240.0, 340.0);

        let sidebar = self.render_sidebar(sidebar_w, cx);
        let editor = self.render_editor(cx);
        let dialog = self.render_delete_dialog(cx);
        let card_menu = self.render_card_menu(cx);

        div()
            .id("memo-root")
            .key_context("Memo")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_new_note))
            .on_action(cx.listener(Self::on_save_now))
            .on_action(cx.listener(Self::on_focus_search))
            .on_action(cx.listener(Self::on_dismiss))
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .font_family(FONT_FAMILY)
            .text_color(TEXT_DARK)
            .bg(BG_EDITOR)
            .child(sidebar)
            .child(editor)
            .children(card_menu)
            .children(dialog)
    }
}
