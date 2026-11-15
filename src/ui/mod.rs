//! UI 层：按职责拆成侧栏、编辑区、弹窗。
//! 三者都通过 `impl NoteApp` 直接访问应用状态，避免层层传参。

pub mod dialogs;
pub mod editor;
pub mod sidebar;
