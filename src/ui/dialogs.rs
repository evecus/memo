//! 弹窗：删除确认。

use eframe::egui::{self, Color32, RichText, Stroke};

use crate::app::NoteApp;
use crate::theme::{ERROR_RED, TEXT_DARK, TEXT_SOFT};

impl NoteApp {
    pub(crate) fn delete_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_delete {
            return;
        }

        let mut open = true;
        let mut decision: Option<bool> = None;
        egui::Window::new("删除便签")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("删除这张便签？")
                        .size(16.0)
                        .strong()
                        .color(TEXT_DARK),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("删除后无法撤销，内容会从数据库中移除。")
                        .size(12.5)
                        .color(TEXT_SOFT),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let del = egui::Button::new(RichText::new("删除").color(Color32::WHITE))
                            .fill(ERROR_RED)
                            .stroke(Stroke::NONE)
                            .rounding(egui::Rounding::same(8.0));
                        if ui.add(del).clicked() {
                            decision = Some(true);
                        }
                        if ui.button("取消").clicked() {
                            decision = Some(false);
                        }
                    });
                });
            });

        match decision {
            Some(true) => self.delete_current(),
            Some(false) => {
                self.confirm_delete = false;
                self.context_menu_note = None;
            }
            None => {
                if !open {
                    self.confirm_delete = false;
                    self.context_menu_note = None;
                }
            }
        }
    }
}
