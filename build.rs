fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");

    // 把 app.ico 作为资源嵌入 exe：GPUI 在 Windows 上创建窗口时读取 exe 内的图标资源，
    // 桌面快捷方式、资源管理器、任务栏也都用它。非 Windows 目标跳过。
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        embed_resource::compile("assets/app.rc", embed_resource::NONE);
    }
}
