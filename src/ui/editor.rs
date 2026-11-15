//! 编辑区：标题、元信息、正文、颜色选择、右键菜单、空状态。
//!
//! 长内容滚动修复：原版把 `TextEdit::multiline` 直接放在面板里，用
//! `desired_rows()` 试图填满窗口。但 egui 的 desired_rows 只是**最小**高度，
//! 实际高度是 `max(内容高度, desired_rows × 行高)`，正文一长就把控件撑出窗口，
//! 超出部分被裁剪且没有滚动条。这里改为放进 `ScrollArea` 并限定 viewport
//! 高度，让内容按全高布局、由滚动区负责滚到底部。

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Stroke, Vec2};

use crate::app::NoteApp;
use crate::theme::{self, ACCENT, ACCENT_SOFT, ERROR_RED, TEXT_DARK, TEXT_FAINT, TEXT_SOFT};
use crate::util::{fmt_ago, fmt_time};

enum ToolAction {
    None,
    Color(usize),
    Delete,
}

impl NoteApp {
    pub(crate) fn editor(&mut self, ui: &mut egui::Ui) {
        self.banner(ui);

        let Some(id) = self.current else {
            self.empty_state(ui);
            return;
        };
        let Some(idx) = self.notes.notes.iter().position(|n| n.id == id) else {
            self.current = None;
            return;
        };

        let cur_color = self.notes.notes[idx].color;
        match self.toolbar(ui, cur_color) {
            ToolAction::Color(i) => self.set_color(i),
            ToolAction::Delete => {
                self.context_menu_note = Some(id);
                self.confirm_delete = true;
            }
            ToolAction::None => {}
        }

        let (title_changed, content_changed, created, updated, chars) = {
            let note = &mut self.notes.notes[idx];

            let t_resp = ui.add(
                egui::TextEdit::singleline(&mut note.title)
                    .font(egui::TextStyle::Heading)
                    .text_color(TEXT_DARK)
                    .hint_text("无标题")
                    .desired_width(f32::INFINITY)
                    .frame(false),
            );
            let title_changed = t_resp.changed();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("创建于 {}", fmt_time(note.created)))
                        .size(12.0)
                        .color(TEXT_FAINT),
                );
                ui.label(RichText::new("·").size(12.0).color(TEXT_FAINT));
                ui.label(
                    RichText::new(fmt_ago(note.updated))
                        .size(12.0)
                        .color(TEXT_FAINT),
                );
            });
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // 预留底部「字数 / 最近编辑」行的高度，避免滚动区把状态行挤出视口。
            let footer_h = ui.text_style_height(&egui::TextStyle::Small) + 18.0;
            let body_h = (ui.available_height() - footer_h).max(80.0);
            let content_id = egui::Id::new(("note_content_editor", id));
            let last_sel_key = egui::Id::new(("note_content_last_selection", id));

            let (content_changed, chars) = egui::ScrollArea::vertical()
                .id_salt(("editor_scroll", id))
                .auto_shrink([false, false])
                .max_height(body_h)
                .show(ui, |ui| {
                    let c_resp = ui.add(
                        egui::TextEdit::multiline(&mut note.content)
                            .id(content_id)
                            .desired_width(f32::INFINITY)
                            .min_size(Vec2::new(ui.available_width(), body_h))
                            .frame(false)
                            .text_color(TEXT_DARK)
                            .hint_text("写点什么…"),
                    );

                    // egui 收到右键点击会清空 TextEdit 选区，因此把最后一次真实
                    // 选区记在独立的 memory 键里，供复制/剪切/粘贴使用。
                    let current_range =
                        egui::widgets::text_edit::TextEditState::load(ui.ctx(), content_id)
                            .and_then(|s| s.cursor.char_range());
                    if let Some(range) = current_range {
                        if range.primary.index != range.secondary.index {
                            ui.ctx().data_mut(|d| d.insert_temp(last_sel_key, range));
                        }
                    }
                    if c_resp.secondary_clicked() {
                        let now_empty =
                            current_range.is_none_or(|r| r.primary.index == r.secondary.index);
                        if now_empty {
                            let saved: Option<egui::text::CCursorRange> =
                                ui.ctx().data(|d| d.get_temp(last_sel_key));
                            if let Some(saved_range) = saved {
                                if let Some(mut state) =
                                    egui::widgets::text_edit::TextEditState::load(
                                        ui.ctx(),
                                        content_id,
                                    )
                                {
                                    state.cursor.set_char_range(Some(saved_range));
                                    state.store(ui.ctx(), content_id);
                                    ui.ctx().request_repaint();
                                }
                            }
                        }
                    }

                    let changed =
                        editor_context_menu(&c_resp, content_id, last_sel_key, &mut note.content)
                            || c_resp.changed();
                    (changed, note.content.chars().count())
                })
                .inner;

            (
                title_changed,
                content_changed,
                note.created,
                note.updated,
                chars,
            )
        };

        if title_changed || content_changed {
            self.mark_dirty();
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{chars} 字"))
                    .size(11.5)
                    .color(TEXT_FAINT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("最近编辑 {}", fmt_ago(updated)))
                        .size(11.5)
                        .color(TEXT_FAINT),
                );
                ui.label(RichText::new("·").size(11.5).color(TEXT_FAINT));
                ui.label(
                    RichText::new(format!("创建 {}", fmt_time(created)))
                        .size(11.5)
                        .color(TEXT_FAINT),
                );
            });
        });
    }

    fn banner(&mut self, ui: &mut egui::Ui) {
        if !self.status_ok {
            egui::Frame::none()
                .fill(Color32::from_rgb(255, 238, 238))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(10.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(self.status.clone())
                            .size(12.5)
                            .color(ERROR_RED),
                    );
                });
            ui.add_space(8.0);
        }

        if let Some(notice) = self.notice.clone() {
            let mut dismiss = false;
            egui::Frame::none()
                .fill(ACCENT_SOFT)
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(notice).size(12.5).color(TEXT_DARK));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("知道了").size(12.0).color(ACCENT),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE),
                                )
                                .clicked()
                            {
                                dismiss = true;
                            }
                        });
                    });
                });
            if dismiss {
                self.notice = None;
            }
            ui.add_space(8.0);
        }
    }

    fn toolbar(&self, ui: &mut egui::Ui, cur_color: usize) -> ToolAction {
        let mut action = ToolAction::None;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("删除").size(12.0).color(TEXT_SOFT))
                            .fill(Color32::from_rgb(243, 245, 250))
                            .stroke(Stroke::NONE)
                            .rounding(egui::Rounding::same(7.0)),
                    )
                    .clicked()
                {
                    action = ToolAction::Delete;
                }
                ui.add_space(8.0);
                for i in 0..theme::NOTE_COLORS.len() {
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::click());
                    let p = ui.painter_at(r);
                    if i == cur_color {
                        p.circle_stroke(r.center(), 8.0, Stroke::new(2.0, ACCENT));
                        p.circle_filled(r.center(), 5.5, theme::color_for(i));
                    } else {
                        p.circle_filled(r.center(), 6.5, theme::color_for(i));
                    }
                    if resp.clicked() {
                        action = ToolAction::Color(i);
                    }
                }
                ui.add_space(4.0);
                ui.label(RichText::new("颜色").size(11.5).color(TEXT_FAINT));
            });
        });
        ui.add_space(8.0);
        action
    }

    fn empty_state(&self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(104.0, 104.0), egui::Sense::hover());
                let p = ui.painter_at(rect);
                let card = Rect::from_min_size(
                    Pos2::new(rect.left() + 14.0, rect.top() + 12.0),
                    Vec2::new(76.0, 80.0),
                );
                p.rect_filled(
                    card.translate(Vec2::new(0.0, 6.0)),
                    egui::Rounding::same(18.0),
                    Color32::from_black_alpha(14),
                );
                p.rect_filled(
                    card,
                    egui::Rounding::same(18.0),
                    Color32::from_rgb(255, 242, 199),
                );
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(card.left() + 16.0, card.top() - 6.0),
                        Pos2::new(card.left() + 38.0, card.top() + 4.0),
                    ),
                    egui::Rounding::same(3.0),
                    ACCENT,
                );
                let line = Color32::from_rgb(192, 178, 128);
                for i in 0..4 {
                    let y = card.top() + 20.0 + i as f32 * 13.0;
                    let w = 46.0 - (i % 2) as f32 * 9.0;
                    p.rect_filled(
                        Rect::from_min_size(Pos2::new(card.left() + 13.0, y), Vec2::new(w, 3.5)),
                        egui::Rounding::same(1.75),
                        line,
                    );
                }

                ui.add_space(16.0);
                ui.label(
                    RichText::new("选择或新建一张便签")
                        .size(16.0)
                        .color(TEXT_SOFT),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Ctrl+N 新建　·　Ctrl+F 搜索　·　Ctrl+S 保存")
                        .size(12.0)
                        .color(TEXT_FAINT),
                );
            });
        });
    }
}

/// 正文右键菜单：复制 / 剪切 / 粘贴 / 全选。
fn editor_context_menu(
    resp: &egui::Response,
    editor_id: egui::Id,
    last_sel_key: egui::Id,
    text: &mut String,
) -> bool {
    use egui::text::{CCursor, CCursorRange};
    use egui::widgets::text_edit::TextEditState;

    let mut changed = false;
    resp.context_menu(|ui| {
        ui.set_min_width(116.0);

        let selected_range: Option<CCursorRange> = ui.ctx().data(|d| d.get_temp(last_sel_key));
        let selected_text: Option<String> = selected_range.and_then(|range| {
            let (min, max) = (
                range.primary.index.min(range.secondary.index),
                range.primary.index.max(range.secondary.index),
            );
            if min == max {
                None
            } else {
                Some(text.chars().skip(min).take(max - min).collect())
            }
        });
        let has_selection = selected_text.as_ref().is_some_and(|s| !s.is_empty());

        if ui
            .add_enabled(has_selection, egui::Button::new("复制"))
            .clicked()
        {
            if let Some(s) = selected_text.clone() {
                ui.output_mut(|o| o.copied_text = s);
            }
            ui.close_menu();
        }

        if ui
            .add_enabled(has_selection, egui::Button::new("剪切"))
            .clicked()
        {
            if let (Some(s), Some(range)) = (selected_text.clone(), selected_range) {
                ui.output_mut(|o| o.copied_text = s);
                let (min, max) = (
                    range.primary.index.min(range.secondary.index),
                    range.primary.index.max(range.secondary.index),
                );
                let before: String = text.chars().take(min).collect();
                let after: String = text.chars().skip(max).collect();
                *text = before + &after;
                changed = true;
                let mut state = TextEditState::load(ui.ctx(), editor_id).unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::one(CCursor::new(min))));
                state.store(ui.ctx(), editor_id);
                ui.ctx()
                    .data_mut(|d| d.remove::<CCursorRange>(last_sel_key));
            }
            ui.close_menu();
        }

        if ui.button("粘贴").clicked() {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(clip_text) = clipboard.get_text() {
                    if !clip_text.is_empty() {
                        let range = selected_range
                            .or_else(|| {
                                TextEditState::load(ui.ctx(), editor_id)
                                    .and_then(|s| s.cursor.char_range())
                            })
                            .unwrap_or_else(|| {
                                CCursorRange::one(CCursor::new(text.chars().count()))
                            });
                        let (min, max) = (
                            range.primary.index.min(range.secondary.index),
                            range.primary.index.max(range.secondary.index),
                        );
                        let before: String = text.chars().take(min).collect();
                        let after: String = text.chars().skip(max).collect();
                        let insert_chars = clip_text.chars().count();
                        *text = before + &clip_text + &after;
                        changed = true;
                        let mut state =
                            TextEditState::load(ui.ctx(), editor_id).unwrap_or_default();
                        state
                            .cursor
                            .set_char_range(Some(CCursorRange::one(CCursor::new(
                                min + insert_chars,
                            ))));
                        state.store(ui.ctx(), editor_id);
                        ui.ctx()
                            .data_mut(|d| d.remove::<CCursorRange>(last_sel_key));
                    }
                }
            }
            ui.close_menu();
        }

        ui.separator();

        if ui.button("全选").clicked() {
            let start = CCursor::new(0);
            let end = CCursor::new(text.chars().count());
            let mut state = TextEditState::load(ui.ctx(), editor_id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(CCursorRange::two(start, end)));
            state.store(ui.ctx(), editor_id);
            ui.ctx()
                .data_mut(|d| d.insert_temp(last_sel_key, CCursorRange::two(start, end)));
            ui.close_menu();
        }
    });
    changed
}
