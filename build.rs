use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // windres 是 MinGW 工具，只在 Windows + GNU 工具链下可用。
    // 原版 build.rs 无条件调用它，导致 Linux/macOS 与 MSVC 目标无法编译。
    // MSVC / 非 Windows 目标跳过 .rc 资源；窗口与任务栏图标仍由运行时
    // ViewportBuilder::with_icon(IconData) 提供，功能不受影响。
    if target_os != "windows" || target_env != "gnu" {
        return;
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR 未设置"));
    let out_file = out.join("app_res.o");
    let status = std::process::Command::new("windres")
        .arg("assets/app.rc")
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&out_file)
        .status()
        .expect("windres 未找到：请确保 MinGW 的 bin 目录在 PATH 中");
    assert!(status.success(), "windres 编译图标资源失败");
    println!("cargo:rustc-link-arg={}", out_file.display());
}
