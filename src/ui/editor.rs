//! 编辑区：提示条、工具栏（颜色/删除）、标题、元信息、正文、底部状态行、空状态。
//!
//! 正文使用 gpui-component 的多行输入框，它自带固定视口 + 垂直滚动：
//! 容器用 `flex_1().min_h_0()` 占满剩余高度，内容再长也只在视口内滚动，
//! 底部「字数 / 最近编辑」行始终可见。复制 / 剪切 / 粘贴 / 全选 / 撤销
//! 由输入框原生提供（快捷键与右键菜单），无需再手工维护选区。

use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight};
use gpui_component::input::Input;

use crate::app::NoteApp;
use crate::theme::{
    self, ACCENT, ACCENT_SOFT, BG_CHIP, BG_CHIP_HOVER, BG_EDITOR, DIVIDER, EDITOR_FONT_SIZE,
    ERROR_RED, ERROR_SOFT, TEXT_DARK, TEXT_FAINT, TEXT_SOFT,
};
use crate::util::{fmt_ago, fmt_time};

impl NoteApp {
    pub(crate) fn render_editor(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let banners = self.render_banners(cx);

        let body = match self.current.and_then(|id| self.notes.get(id)) {
            None => render_empty_state(),
            Some(note) => {
                let color = note.color;
                let created = note.created;
                let updated = note.updated;
                let chars = note.content.chars().count();
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.render_toolbar(color, cx))
                    // 标题
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(Input::new(&self.title_input).appearance(false)),
                    )
                    // 元信息
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .pt(px(4.0))
                            .text_size(px(12.0))
                            .text_color(TEXT_FAINT)
                            .child(format!("创建于 {}", fmt_time(created)))
                            .child("·")
                            .child(fmt_ago(updated)),
                    )
                    .child(div().my(px(10.0)).h(px(1.0)).w_full().bg(DIVIDER))
                    // 正文：占满剩余高度，内部滚动
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .text_size(px(EDITOR_FONT_SIZE))
                            .child(
                                Input::new(&self.content_input)
                                    .appearance(false)
                                    .h_full(),
                            ),
                    )
                    // 底部状态行
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .justify_between()
                            .pt(px(8.0))
                            .text_size(px(11.5))
                            .text_color(TEXT_FAINT)
                            .child(format!("{chars} 字 · {}", self.status))
                            .child(format!(
                                "创建 {} · 最近编辑 {}",
                                fmt_time(created),
                                fmt_ago(updated)
                            )),
                    )
                    .into_any_element()
            }
        };

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .p(px(20.0))
            .bg(BG_EDITOR)
            .children(banners)
            .child(body)
            .into_any_element()
    }

    fn render_banners(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = Vec::new();

        if !self.status_ok {
            out.push(
                div()
                    .flex_none()
                    .mb(px(8.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(ERROR_SOFT)
                    .text_size(px(12.5))
                    .text_color(ERROR_RED)
                    .child(self.status.clone())
                    .into_any_element(),
            );
        }

        if let Some(notice) = self.notice.clone() {
            out.push(
                div()
                    .flex_none()
                    .mb(px(8.0))
                    .p(px(10.0))
                    .rounded(px(8.0))
                    .bg(ACCENT_SOFT)
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(12.5))
                    .text_color(TEXT_DARK)
                    .child(notice)
                    .child(
                        div()
                            .id("notice-dismiss")
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded(px(6.0))
                            .text_size(px(12.0))
                            .text_color(ACCENT)
                            .hover(|s| s.bg(BG_EDITOR))
                            .cursor_pointer()
                            .child("知道了")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.notice = None;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            );
        }
        out
    }

    fn render_toolbar(&self, cur_color: usize, cx: &mut Context<Self>) -> AnyElement {
        let mut dots = div().flex().items_center().gap(px(6.0));
        for i in 0..theme::NOTE_COLORS.len() {
            let selected = i == cur_color;
            dots = dots.child(
                div()
                    .id(("color", i))
                    .size(px(18.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(selected, |s| s.border_2().border_color(ACCENT))
                    .child(
                        div()
                            .size(px(if selected { 10.0 } else { 13.0 }))
                            .rounded_full()
                            .bg(theme::color_for(i)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.set_color(i, cx))),
            );
        }

        let current = self.current;
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.0))
            .pb(px(8.0))
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(TEXT_FAINT)
                    .child("颜色"),
            )
            .child(dots)
            .child(
                div()
                    .id("delete-note")
                    .ml(px(8.0))
                    .px(px(10.0))
                    .py(px(4.0))
                    .rounded(px(7.0))
                    .bg(BG_CHIP)
                    .hover(|s| s.bg(BG_CHIP_HOVER))
                    .cursor_pointer()
                    .text_size(px(12.0))
                    .text_color(TEXT_SOFT)
                    .child("删除")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(id) = current {
                            this.request_delete(id, cx);
                        }
                    })),
            )
            .into_any_element()
    }
}

/// 空状态：便签插画 + 提示文字。
fn render_empty_state() -> AnyElement {
    let line = gpui::rgb(0xC0B280);
    let mut lines = div()
        .absolute()
        .left(px(13.0))
        .top(px(20.0))
        .flex()
        .flex_col()
        .gap(px(9.5));
    for i in 0..4 {
        let w = 46.0 - (i % 2) as f32 * 9.0;
        lines = lines.child(div().w(px(w)).h(px(3.5)).rounded(px(1.75)).bg(line));
    }

    let illustration = div()
        .relative()
        .size(px(104.0))
        // 阴影
        .child(
            div()
                .absolute()
                .left(px(14.0))
                .top(px(18.0))
                .w(px(76.0))
                .h(px(80.0))
                .rounded(px(18.0))
                .bg(gpui::rgba(0x0000000E)),
        )
        // 纸片
        .child(
            div()
                .absolute()
                .left(px(14.0))
                .top(px(12.0))
                .w(px(76.0))
                .h(px(80.0))
                .rounded(px(18.0))
                .bg(gpui::rgb(0xFFF2C7))
                .child(lines),
        )
        // 胶带
        .child(
            div()
                .absolute()
                .left(px(30.0))
                .top(px(6.0))
                .w(px(22.0))
                .h(px(10.0))
                .rounded(px(3.0))
                .bg(ACCENT),
        );

    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .child(illustration)
        .child(
            div()
                .mt(px(10.0))
                .text_size(px(16.0))
                .text_color(TEXT_SOFT)
                .child("选择或新建一张便签"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(TEXT_FAINT)
                .child("Ctrl+N 新建　·　Ctrl+F 搜索　·　Ctrl+S 保存"),
        )
        .into_any_element()
}
