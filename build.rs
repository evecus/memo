fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");

    // 把 app.ico 作为资源嵌入 exe，桌面快捷方式、资源管理器和任务栏固定项
    // 才会显示自定义图标。embed-resource 同时支持 MSVC（rc.exe）与
    // GNU（windres）工具链，非 Windows 目标自动跳过。
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        embed_resource::compile("assets/app.rc", embed_resource::NONE);
    }
}
