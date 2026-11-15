#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod db;
mod model;
mod theme;
mod ui;
mod util;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Memo")
        .with_inner_size([1180.0, 660.0])
        .with_min_inner_size([620.0, 420.0]);
    if let Some(icon) = app::load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Memo",
        options,
        // 数据库初始化失败会沿 ? 传播，由 eframe 报错退出（fail-fast）。
        Box::new(|cc| Ok(Box::new(app::NoteApp::new(cc)?) as Box<dyn eframe::App>)),
    )
}
