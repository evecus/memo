//! 弹窗：删除确认。全窗口半透明遮罩 + 居中卡片；点击遮罩或按 Esc 取消。

use gpui::{deferred, div, prelude::*, px, AnyElement, Context, FontWeight};

use crate::app::NoteApp;
use crate::theme::{
    BG_CHIP, BG_CHIP_HOVER, BG_EDITOR, ERROR_RED, OVERLAY, TEXT_DARK, TEXT_SOFT,
};

impl NoteApp {
    pub(crate) fn render_delete_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.confirm_delete?;

        let card = div()
            .id("delete-dialog")
            .occlude()
            .w(px(340.0))
            .p(px(20.0))
            .rounded(px(12.0))
            .bg(BG_EDITOR)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // 阻止点击卡片本身时冒泡到遮罩
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(TEXT_DARK)
                    .child("删除这张便签？"),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(TEXT_SOFT)
                    .child("删除后无法撤销，内容会从数据库中移除。"),
            )
            .child(
                div()
                    .mt(px(14.0))
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id("dialog-cancel")
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(8.0))
                            .bg(BG_CHIP)
                            .hover(|s| s.bg(BG_CHIP_HOVER))
                            .cursor_pointer()
                            .text_size(px(13.0))
                            .child("取消")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_delete = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("dialog-delete")
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(8.0))
                            .bg(ERROR_RED)
                            .hover(|s| s.opacity(0.9))
                            .cursor_pointer()
                            .text_size(px(13.0))
                            .text_color(BG_EDITOR)
                            .child("删除")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.delete_confirmed(window, cx);
                            })),
                    ),
            );

        Some(
            deferred(
                div()
                    .id("delete-overlay")
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(OVERLAY)
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_delete = None;
                        cx.notify();
                    }))
                    .child(card),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}
