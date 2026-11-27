# Memo（Rust + GPUI / redb 版）

把 Memo 的界面层从 **egui（eframe 0.29，即时模式）** 改写到 **GPUI（Zed 编辑器的 GPU 加速 UI 框架，保留模式）**。存储层（redb）、数据模型、时间工具原样保留，所有交互都保留：搜索、卡片悬停、右键删除、700ms 防抖 / 5s 兜底自动保存、恢复上次便签、删除确认、颜色选择、快捷键、旧 `notes.json` 迁移。

## 一、技术栈

| 层 | 旧版 | 本版 |
|---|---|---|
| UI 框架 | `eframe` / `egui` 0.29 | `gpui` 0.2.2（crates.io） |
| 文本输入 | `egui::TextEdit` + 手写右键菜单/选区恢复 | `gpui-component` 0.4 的 `InputState` / `Input`（单行 + 多行） |
| 剪贴板 | `arboard` | 输入框原生（系统剪贴板） |
| 图标解码 | `image`（窗口图标 + Logo 纹理） | 去掉；Windows 图标走 exe 资源，Logo 改为矢量绘制 |
| 存储 | redb 2 | redb 2（未改动） |
| 渲染后端 | OpenGL (glow) | macOS Metal / Windows DirectX / Linux Vulkan |

## 二、源码结构

| 文件 | 职责 | 和 egui 版的区别 |
|---|---|---|
| `src/main.rs` | 打开数据库（fail-fast）、注册字体/主题/快捷键、开窗口、套 `Root` | 数据库在开窗口前打开，失败直接退出 |
| `src/app.rs` | `NoteApp` 状态、输入框实体、事件订阅、自动保存定时器、Action、`Render` | 每帧轮询 → 事件驱动 + 后台定时器 |
| `src/theme.rs` | 配色常量（`Rgba`）、字体注册、强制浅色 | 删除字体图集预热代码 |
| `src/ui/sidebar.rs` | Logo、新建、搜索、卡片列表、卡片右键菜单 | 卡片是 `div` + `hover` 样式；菜单用 `deferred(anchored())` |
| `src/ui/editor.rs` | 提示条、颜色/删除工具栏、标题、元信息、正文、底部状态、空状态 | 正文是多行 `Input`，`flex_1().min_h_0()` 铺满剩余高度 |
| `src/ui/dialogs.rs` | 删除确认弹窗 | 全窗口遮罩 + 居中卡片，点遮罩/Esc 取消 |
| `src/db.rs` / `src/model.rs` / `src/util.rs` | redb 存储、模型、时间/字符串工具 | **未改动**，测试照旧 |

## 三、架构变化要点

**1. 即时模式 → 保留模式**
egui 每帧重新跑一遍 `update()`，直接改 `&mut String`。GPUI 里状态放在 `Entity` 中，`render()` 只在 `cx.notify()` 后重新构建元素树。三个输入框（搜索/标题/正文）各是一个 `Entity<InputState>`，通过 `cx.subscribe_in` 监听 `InputEvent::Change`，再回写到 `Note`。

**2. 输入框与模型同步**
切换便签时用 `set_value` 把内容写入输入框。`Change` 回调里会先**比较输入框值与模型值**，一样就不标脏，所以程序自己写入的内容不会被当成用户编辑、也不会触发多余的保存。

**3. 自动保存**
`cx.spawn` 起一个后台循环，每 250ms 检查一次「700ms 防抖 / 5s 兜底」，每 1s `notify` 一次刷新「N 分钟前」。另外三处兜底：切换/新建便签前先保存，`on_app_quit` 保存，视图释放（关窗口）时 `on_release` 保存。

**4. 快捷键**
用 GPUI 的 `actions!` + `KeyBinding`，`secondary-n/s/f` 在 macOS 上是 ⌘，其它平台是 Ctrl；`escape` 关闭弹窗和菜单。

**5. egui 版那两个修复在这里不再需要**
- *启动闪烁*：根因是 egui 字体图集从 32px 起步、填充超过 80% 就整体重建。GPUI 的字形图集是按需分配纹理页的，不会整体重建，所以预热帧、`warmup_fonts`、`WARMUP_STATIC_TEXT` 全部删掉。
- *长内容滚不到底*：多行 `Input` 自带视口和垂直滚动，容器用 flex 约束高度，底部状态行一直可见。
- *右键菜单丢选区*：egui 右键会清空 TextEdit 选区，所以旧版手动保存/恢复选区。gpui-component 的输入框原生支持复制/剪切/粘贴/全选/撤销，这段约 150 行的代码删掉了。

## 四、数据与兼容

数据库格式**完全不变**：老用户直接升级，`%APPDATA%\Memo\memo.redb`（Linux：`$XDG_DATA_HOME/memo/memo.redb`）原样读取；更早的 `notes.json` 仍会自动迁移并备份成 `notes.json.migrated.bak`。

## 五、构建

```bash
# 需要 Rust 1.85+
cargo build --release

# Linux 依赖（Debian/Ubuntu）
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libx11-xcb-dev libxcb1-dev libvulkan-dev libfontconfig-dev libfreetype-dev \
  pkg-config clang
cargo run
```

> 旧的 `Cargo.lock` 是 egui 的依赖树，已删除。第一次构建会重新生成；确认能编译后提交它，再把 CI 里的 `--locked` 加回来。

Windows：`build.rs` 通过 `embed-resource` 把 `assets/app.ico` 嵌进 exe，GPUI 创建窗口时用这个资源作为窗口/任务栏图标。NSIS 安装脚本 `install.nsi` 不变。

## 六、快捷键与操作

| 操作 | 方式 |
|---|---|
| 新建便签 | `Ctrl+N`（macOS `⌘N`）或侧栏按钮 |
| 立即保存 | `Ctrl+S`（平时自动保存） |
| 聚焦搜索 | `Ctrl+F` |
| 关闭弹窗 / 菜单 | `Esc` |
| 删除便签 | 卡片右键 → 删除便签，或编辑区右上「删除」 |
| 复制 / 剪切 / 粘贴 / 全选 / 撤销 | 正文区快捷键或右键菜单（输入框原生） |

## 七、注意：GPUI 生态版本变化快

GPUI 还没到 1.0，`gpui-component` 基本每周发版、经常有破坏性改动。本项目按 `gpui 0.2.2` + `gpui-component 0.4` 的 API 编写。如果你本地解析到别的版本、编译报错，通常只是这几处改了名：

| 可能报错的位置 | 可能的新/旧写法 |
|---|---|
| `gpui_component::input::Input` | 旧版叫 `TextInput` |
| `InputState::multi_line()` | 新版可能是 `.multi_line(true)` |
| `Root::new(view, window, cx)` | 参数类型在 `AnyView` / `impl Into<AnyView>` 之间变过 |
| `Theme::change(ThemeMode::Light, None, cx)` | 新版第二个参数可能是 `Option<&mut Window>` |
| `f32::from(Pixels)` | 也可以写 `.as_f32()` / `.0` |
