//! 侧栏：Logo、新建按钮、搜索、便签卡片列表、右键删除菜单。

use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};

use crate::app::NoteApp;
use crate::theme::{self, ACCENT, ERROR_RED, TEXT_DARK, TEXT_FAINT, TEXT_SOFT};
use crate::util::{first_line, fmt_time, truncate};

const CARD_HEIGHT: f32 = 80.0;

impl NoteApp {
    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        self.header(ui);

        let new_btn =
            egui::Button::new(RichText::new("＋  新建便签").color(Color32::WHITE).strong())
                .fill(ACCENT)
                .stroke(Stroke::NONE)
                .rounding(egui::Rounding::same(10.0))
                .min_size(Vec2::new(ui.available_width(), 36.0));
        let resp = ui.add(new_btn);
        if resp.clicked() {
            self.new_note();
        }
        if resp.hovered() {
            resp.on_hover_text("快捷键 Ctrl+N");
        }
        ui.add_space(10.0);

        let search_resp = ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .id(egui::Id::new("sidebar_search"))
                .hint_text("搜索便签…")
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::same(9.0)),
        );
        if self.focus_search {
            search_resp.request_focus();
            self.focus_search = false;
        }
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("全部便签")
                    .size(12.0)
                    .strong()
                    .color(TEXT_SOFT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} 张", self.notes.notes.len()))
                        .size(11.5)
                        .color(TEXT_FAINT),
                );
            });
        });
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.card_list(ui));
    }

    fn header(&self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if let Some(tex) = &self.logo_texture {
                ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::new(20.0, 20.0)));
            } else {
                let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 9.0, ACCENT);
                ui.painter()
                    .circle_filled(r.center() - Vec2::new(1.5, 1.5), 4.5, Color32::WHITE);
            }
            ui.add_space(5.0);
            ui.label(RichText::new("Memo").size(21.0).strong().color(TEXT_DARK));
        });
        ui.add_space(3.0);
        ui.label(
            RichText::new("轻量的本地便签 · 自动保存")
                .size(11.5)
                .color(TEXT_FAINT),
        );
        ui.add_space(16.0);
    }

    fn card_list(&mut self, ui: &mut egui::Ui) {
        let ids = self.notes.sorted_ids(&self.search);
        if ids.is_empty() {
            ui.add_space(36.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(if self.search.trim().is_empty() {
                        "还没有便签"
                    } else {
                        "没有匹配的便签"
                    })
                    .size(14.0)
                    .color(TEXT_SOFT),
                );
            });
            return;
        }

        let mut clicked: Option<u64> = None;
        let mut delete_requested: Option<u64> = None;

        for id in ids {
            let Some((title, preview, updated, color_idx)) = self.notes.get(id).map(|n| {
                (
                    n.display_title(),
                    first_line(&n.content),
                    n.updated,
                    n.color,
                )
            }) else {
                continue;
            };
            let selected = self.current == Some(id);

            let (rect, resp) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), CARD_HEIGHT),
                egui::Sense::click(),
            );
            let hovered = resp.hovered() || resp.dragged();
            let t = ui.ctx().animate_bool_with_time(
                egui::Id::new(("card", id)),
                hovered || selected,
                0.16,
            );

            let base = theme::color_for(color_idx);
            let fill = theme::lerp_color(base.gamma_multiply(0.88), base, t);
            let border = if selected {
                Stroke::new(1.8, ACCENT)
            } else {
                Stroke::new(1.0, Color32::from_black_alpha(12))
            };

            let painter = ui.painter_at(rect);
            let lift = theme::lerp(1.5, 3.5, t);
            let shadow_alpha = theme::lerp(10.0, 28.0, t) as u8;
            painter.rect_filled(
                rect.translate(Vec2::new(0.0, lift)),
                egui::Rounding::same(12.0),
                Color32::from_black_alpha(shadow_alpha),
            );
            painter.rect_filled(rect, egui::Rounding::same(12.0), fill);
            painter.rect_stroke(rect, egui::Rounding::same(12.0), border);

            if selected {
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(rect.left() + 5.0, rect.top() + 15.0),
                        Pos2::new(rect.left() + 8.0, rect.bottom() - 15.0),
                    ),
                    egui::Rounding::same(1.5),
                    ACCENT,
                );
            }

            let x = rect.left() + 16.0;
            let w = rect.width() - 32.0;
            let title_galley = painter.layout(
                truncate(&title, 20),
                FontId::proportional(15.0),
                TEXT_DARK,
                w,
            );
            painter.galley(Pos2::new(x, rect.top() + 10.0), title_galley, TEXT_DARK);

            if !preview.is_empty() {
                let g = painter.layout(
                    truncate(&preview, 34),
                    FontId::proportional(12.5),
                    TEXT_SOFT,
                    w,
                );
                painter.galley(Pos2::new(x, rect.top() + 33.0), g, TEXT_SOFT);
            }

            let tg = painter.layout(
                fmt_time(updated),
                FontId::proportional(11.0),
                TEXT_FAINT,
                90.0,
            );
            painter.galley(
                Pos2::new(
                    rect.right() - tg.size().x - 12.0,
                    rect.bottom() - tg.size().y - 7.0,
                ),
                tg,
                TEXT_FAINT,
            );

            if resp.clicked() {
                clicked = Some(id);
            }

            let mut want_delete = false;
            resp.context_menu(|ui| {
                ui.set_min_width(100.0);
                if ui
                    .add(egui::Button::new(
                        RichText::new("删除便签").color(ERROR_RED),
                    ))
                    .clicked()
                {
                    want_delete = true;
                    ui.close_menu();
                }
            });
            if want_delete {
                delete_requested = Some(id);
            }

            ui.add_space(7.0);
        }

        if let Some(id) = clicked {
            self.current = Some(id);
            self.confirm_delete = false;
            // 切换后立即记录，关闭时不编辑也能恢复到这张便签。
            self.save();
        }
        if let Some(id) = delete_requested {
            self.context_menu_note = Some(id);
            self.confirm_delete = true;
        }
    }
}
