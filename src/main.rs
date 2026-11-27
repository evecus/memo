#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod db;
mod model;
mod theme;
mod ui;
mod util;

use gpui::{
    px, size, AnyView, AppContext, Application, Bounds, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use gpui_component::Root;

fn main() {
    // fail-fast：数据库打不开 / 读不出直接退出，不静默降级成空便签。
    // 放在窗口创建之前，避免 UI 起来后才发现存储不可用。
    let db = match db::Db::open() {
        Ok(db) => db,
        Err(e) => fatal(&format!("无法打开便签数据库：{e}")),
    };
    let notes = match db.load() {
        Ok(notes) => notes,
        Err(e) => fatal(&format!("读取便签数据库失败：{e}")),
    };

    Application::new().run(move |cx| {
        gpui_component::init(cx);
        theme::init(cx);
        app::bind_keys(cx);

        // GPUI 在 Windows / Linux 上关闭最后一个窗口不会自动退出进程。
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1180.0), px(660.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(620.0), px(420.0))),
            titlebar: Some(TitlebarOptions {
                title: Some("Memo".into()),
                ..Default::default()
            }),
            app_id: Some("memo".into()),
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let view = cx.new(|cx| app::NoteApp::new(db, notes, window, cx));
            // gpui-component 的输入框、弹层依赖 Root 作为窗口根视图。
            let view: AnyView = view.into();
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("无法创建窗口");

        cx.activate(true);
    });
}

fn fatal(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}
