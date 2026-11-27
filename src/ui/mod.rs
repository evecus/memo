//! UI 层：按职责拆成侧栏、编辑区、弹窗。
//! 三者都通过 `impl NoteApp` 直接访问应用状态，每个函数返回一棵 GPUI 元素树。

pub mod dialogs;
pub mod editor;
pub mod sidebar;
