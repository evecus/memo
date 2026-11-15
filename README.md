# Memo（Rust + egui / redb 版）

把原 `memo-master` 用 Rust + egui（eframe 0.29）重写：**存储层从 `notes.json` 全量读写换成 redb 2.6 嵌入式数据库**，按职责拆分源码、美化界面，并修掉「启动后闪几下」与「便签内容过长看不到底部」两个问题。原有交互（搜索、卡片动效、右键菜单、自动保存防抖、恢复上次便签、删除确认、快捷键）全部保留。

## 一、与旧版对比

| 维度 | 旧版 | 本版 |
|---|---|---|
| 存储 | `%APPDATA%\Memo\notes.json`，每次保存把全部便签序列化后整体重写 | `memo.redb` 单文件数据库，一次事务写「当前便签 + 元数据」，不再整体重写 |
| 崩溃一致性 | 自己写 `notes.json.tmp` + `rename` | redb 事务提交，原子且带 MVCC |
| 读失败处理 | `unwrap_or_default()`，坏文件静默变成空便签 | 数据库打开/读取失败由 `?` 传播，程序报错退出，不静默降级 |
| 源码结构 | `src/main.rs` 单文件 896 行 | 拆成 10 个文件、6 个职责模块 |
| 启动闪烁 | 启动后几秒闪几次，之后稳定 | 启动预热帧 + 字体图集预热 + 重绘降频，已修复 |
| 长内容滚动 | 正文被内容撑出窗口，超出部分看不到、没有滚动条 | 正文放进 `ScrollArea`，可滚到最后一行 |
| 界面 | 基础卡片布局 | 圆角/阴影层次、副标题、分组标题、颜色选择器、提示条、弹窗与空状态重绘 |
| 旧数据 | 无 | 首次启动自动导入 `notes.json` 并备份 |
| `build.rs` | 无条件调用 `windres`，Linux/macOS/MSVC 无法编译 | 按 `CARGO_CFG_TARGET_OS/TARGET_ENV` 条件执行 |
| CI | 仅 Windows release 构建 | fmt / `clippy --locked -D warnings` / `cargo test` + Windows release + NSIS |

## 二、源码拆分

旧版所有逻辑（主题、时间工具、数据模型、存储、侧栏、编辑区、右键菜单、弹窗、入口）都堆在 `src/main.rs`。现在按职责分开：

| 文件 | 职责 | 旧版对应位置 |
|---|---|---|
| `src/main.rs` | 入口、窗口配置、图标、启动应用 | main.rs 入口部分 |
| `src/app.rs` | `NoteApp` 状态、自动保存调度、快捷键、启动预热、面板编排、`eframe::App` | main.rs 的 `NoteApp` 与 `update` |
| `src/db.rs` | redb 建表 / CRUD / 元数据 / 旧 JSON 迁移 | main.rs 的 `data_path` / `load_notes` / `save` |
| `src/model.rs` | `Note` / `Notes`、排序与过滤 | main.rs 数据模型 |
| `src/theme.rs` | 配色、字体/字号/视觉初始化、启动字体预热 | main.rs 主题与字体段 |
| `src/util.rs` | 时间格式化、相对时间、截断、首行摘要 | main.rs 工具函数 |
| `src/ui/sidebar.rs` | Logo、新建、搜索、卡片列表、右键删除 | main.rs `sidebar` |
| `src/ui/editor.rs` | 标题/元信息/正文/颜色选择/右键菜单/空状态 | main.rs `editor`、`editor_context_menu`、`empty_state` |
| `src/ui/dialogs.rs` | 删除确认弹窗 | main.rs `update` 里的弹窗段 |
| `build.rs` | Windows 图标资源（条件编译） | build.rs |

## 三、数据库设计

| 表 | key | value | 说明 |
|---|---|---|---|
| `notes` | `u64`（便签 id） | `&[u8]`（`Note` 的 JSON 字节） | 一条便签一个 key，改动只写这一条 |
| `meta` | `&str` | `&str` | `next_id` / `last_open` / `schema_version` |

- **数据位置**：Windows `%APPDATA%\Memo\memo.redb`；Linux `$XDG_DATA_HOME/memo/memo.redb`（缺省 `$HOME/.local/share/memo/`）。
- **写入时机**：新建、切换便签、换颜色、删除立即落库；编辑时按 **700ms 防抖 / 5s 兜底** 触发；退出时再保存一次。
- **一次保存一个事务**：`Db::save_state(note, last_open, next_id)` 用单个写事务同时写当前便签与两个元数据，比拆成三个事务更省 IO，也避免中途崩溃造成状态不一致。
- **fail-fast**：`Db::open()?` / `db.load()?` 的错误沿 `AppCreator` 的 `Result` 向上传播；写失败会在编辑区顶部显示红色错误条，而不是假装保存成功。

### 旧数据迁移

首次启动时若 `notes` 表为空且 `notes.json` 存在：

1. 读旧 JSON，用一个写事务导入全部便签、`next_id`、`last_open`；
2. `next_id` 用 `max(旧 next_id, 最大 id + 1)` 校正，避免新建时覆盖已有记录；
3. 成功后把 `notes.json` 重命名为 `notes.json.migrated.bak`（保留可回溯备份，同时避免下次重复导入）；
4. 界面顶部显示一次性迁移提示。卸载程序不会删除 `memo.redb` 与该备份。

## 四、启动闪烁：根因与修复

用户现象：**只在启动后几秒闪几次，之后不再闪**。这不是持续的布局震荡，而是启动期的字体图集反复重建，证据在 egui/epaint 0.29 源码里：

- `epaint/src/text/fonts.rs`：`FontsImpl::new` 把字体图集初始高度设为 **32px**（注释就写着 "Keep initial font atlas small … This will expand as needed"）；
- 同一个文件的 `Fonts::begin_pass`：当 `atlas.fill_ratio() > 0.8` 时，**重建整个 `FontsAndCache`**（字形缓存与 galley 缓存全部清空）；
- `Context::set_fonts` 的文档：新字体"在下一趟 pass 开始时才生效"。

于是启动这一刻要同时栅格化内嵌中文子集里的全部界面文案，加上启动时恢复的 `last_open` 便签正文。新字形逐帧加入 → 图集连续扩容 / `begin_pass` 反复重建 → 每次纹理重新上传就闪一下；等所有字形都缓存好，图集稳定，闪烁自然停止。旧版 `ctx.request_repaint_after(300ms)` 无条件重绘，正好让每一次重建都可见。此外旧版正文高度由 `ui.available_height()` 每帧推导，窗口尺寸/DPI 在启动前几帧稳定下来之前也会让布局跳变。

修复（三层，都作用在启动期）：

1. **启动预热帧**：`NoteApp::update` 的最初若干帧只画纯色背景，不渲染任何文字或纹理，把图集扩容、纹理重建、窗口几何稳定全部挡在用户可见内容之外。
2. **字体一次性预热**：`theme::warmup_fonts` 把界面全部静态文案 + **所有便签的标题与正文** 去重后，按界面用到的 8 个字号一次性 `layout_no_wrap`，让图集一次扩容到位。用「本帧填充率是否比上一帧末尾骤降」判断字体对象是否被重建，连续 2 帧稳定或最多 12 帧后进入正常 UI。
   > `Context::fonts()` 在第一次 `Context::run()` 之前不可用（源码有 `expect("No fonts available until first call to Context::run()")`），所以预热必须放在 `update` 首帧，不能放在 `NoteApp::new`。
3. **降频**：相对时间刷新由 300ms 改为 1s；正文高度改为显式预留 + `ScrollArea` 固定 viewport，不再逐帧推导。

| | 修复前 | 修复后 |
|---|---|---|
| 启动前几帧 | 直接渲染完整 UI，图集边扩容边重建 | 纯色背景 + 字形预热，纹理重建不可见 |
| 字形进入图集 | 随 UI 绘制逐帧增量加入 | 首帧按全部文案/便签内容一次性排版 |
| 重绘节奏 | 每 300ms 无条件重绘 | 预热期连续帧（最多 12 帧），之后 1s 一次 |
| 正文高度 | `available_height()` 每帧推导 | 固定 viewport 高度，布局稳定 |

通常情况下预热只需 2～3 帧（约 30～50ms），用户看到的是窗口底色一闪而过，而不是内容闪烁。

## 五、长内容不能滚动：根因与修复

根因同样在 egui 源码 `egui/src/widgets/text_edit/builder.rs:520-521`：

```rust
let desired_height = (desired_height_rows.at_least(1) as f32) * row_height;
let desired_inner_size = vec2(desired_width, galley.size().y.max(desired_height));
```

`desired_rows(n)` 只是**最小**高度，不是固定高度。控件实际高度 = `max(内容排版高度, n 行)`，所以正文一长，TextEdit 就被撑到内容全高、直接超出窗口。旧版没有把它放进 `ScrollArea`，超出部分被 `CentralPanel` 裁掉，既看不到底部也没有滚动条——正好是「只能显示一个窗口大小的内容」。

修复：正文改为放进滚动区，并显式预留底部状态行的高度：

```rust
egui::ScrollArea::vertical()
    .id_salt(("editor_scroll", id))
    .auto_shrink([false, false])
    .max_height(body_h)   // body_h = 可用高度 - 底部行高度
    .show(ui, |ui| ui.add(
        egui::TextEdit::multiline(&mut note.content)
            .desired_width(f32::INFINITY)
            .min_size(Vec2::new(ui.available_width(), body_h)) // 内容少时铺满编辑区
            .frame(false)
    ));
```

| | 修复前 | 修复后 |
|---|---|---|
| 高度决定 | `desired_rows((可用高度/行高).max(8))`，实为内容高度 | `ScrollArea` 固定 viewport，内容按全高布局 |
| 超出窗口 | 被裁剪，无滚动条 | 出现垂直滚动条，可滚到最后一行 |
| 底部字数行 | 可能被正文挤出视口 | 事先预留 `footer_h`，始终可见 |
| 切换便签 | 高度随内容突变 | viewport 高度恒定，不跳变 |

## 六、界面美化

- 侧栏：Logo + 应用名 + 「轻量的本地便签 · 自动保存」副标题；全宽圆角「＋ 新建便签」按钮；搜索框；「全部便签」分组行与计数。
- 卡片：圆角 12px、双层柔和阴影随 hover 加深并轻微上浮、选中指示条、标题/摘要/时间三段式排版。
- 编辑区：标题字号加大、创建时间与相对时间分行、分隔线、**6 色便签颜色选择器**（点击即写库）、底部「字数 · 创建 · 最近编辑」。
- 弹窗与反馈：删除确认弹窗重绘；数据库写失败显示红色错误条；迁移完成显示蓝色提示条；空状态便签插画与提示文字重排。
- 交互补充：`Ctrl+F` 聚焦搜索；强制浅色主题不跟随系统深色模式；中文字体内嵌，中英文统一渲染。

## 七、构建与 CI

```bash
# Windows（本地或 CI）
cargo build --release

# Linux 需先装图形依赖
sudo apt-get install -y libxkbcommon-dev libwayland-dev libx11-dev \
  libxcb1-dev libxrandr-dev libxi-dev libgl1-mesa-dev
cargo run
```

`build.rs` 只在 `windows + gnu` 工具链调用 `windres` 嵌入 `.rc` 图标；MSVC 与非 Windows 目标跳过（窗口/任务栏图标仍由运行时 `ViewportBuilder::with_icon` 提供）。

CI（`.github/workflows/build.yml`）：`cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked`，随后在 Windows 上构建 release、上传 exe，`workflow_dispatch` 时再用 NSIS 打包并发布 Release。

## 八、快捷键与操作

| 操作 | 方式 |
|---|---|
| 新建便签 | `Ctrl+N` 或侧栏按钮 |
| 立即保存 | `Ctrl+S`（平时自动保存） |
| 聚焦搜索 | `Ctrl+F` |
| 关闭弹窗 | `Esc` |
| 删除便签 | 卡片右键 → 删除，或编辑区右上「删除」 |
| 复制 / 剪切 / 粘贴 / 全选 | 正文区右键菜单 |

## 九、验证记录（沙箱 Linux / Xvfb 软件渲染）

| 项目 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --locked --all-targets -- -D warnings` | 通过，零警告 |
| `cargo test --locked` | 6 个测试全部通过（模型排序、中文截断、日期换算、redb CRUD、旧 JSON 迁移） |
| 真实启动 | 连续运行无 panic、无报错日志 |
| 旧数据迁移 | `notes.json` 自动导入为 `memo.redb`，并生成 `notes.json.migrated.bak` |
| 长内容滚动 | 200 行 / 9091 字便签，滚轮可滚到第 200 行，底部状态行始终可见 |

## 十、依赖与取舍

依赖：`eframe`/`egui` 0.29、`redb` 2.6、`serde`/`serde_json`、`arboard` 3、`image` 0.25（均纯 Rust 依赖树，除系统图形库外无 C 代码需要编译）。

取舍说明：启动预热会在最开始若干帧只显示纯色背景（最多 12 帧、约 200ms，通常 2～3 帧），这是把字体纹理重建挡在可见内容之外的代价；MSVC 构建不嵌入 exe 的 `.rc` 图标资源，需要资源管理器图标时可改用 `x86_64-pc-windows-gnu` 目标。
