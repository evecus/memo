//! 侧栏：Logo、新建按钮、搜索、便签卡片列表、卡片右键菜单。

use gpui::{
    anchored, deferred, div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton,
    MouseDownEvent,
};
use gpui_component::input::Input;

use crate::app::NoteApp;
use crate::theme::{
    self, ACCENT, ACCENT_HOVER, BG_EDITOR, BG_SIDEBAR, CARD_BORDER, DIVIDER, ERROR_RED,
    ERROR_SOFT, TEXT_DARK, TEXT_FAINT, TEXT_SOFT,
};
use crate::util::truncate;

const CARD_HEIGHT: f32 = 52.0;

impl NoteApp {
    pub(crate) fn render_sidebar(&mut self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let total = self.notes.notes.len();

        div()
            .w(px(width))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .p(px(14.0))
            .gap(px(10.0))
            .bg(BG_SIDEBAR)
            .border_r_1()
            .border_color(DIVIDER)
            // Logo + 应用名 + 副标题
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .pt(px(6.0))
                    .pb(px(4.0))
                    .child(
                        div()
                            .size(px(34.0))
                            .rounded(px(10.0))
                            .bg(ACCENT)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(BG_EDITOR)
                            .text_size(px(18.0))
                            .font_weight(FontWeight::BOLD)
                            .child("M"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Memo"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(TEXT_FAINT)
                                    .child("轻量的本地便签 · 自动保存"),
                            ),
                    ),
            )
            // 新建按钮
            .child(
                div()
                    .id("new-note")
                    .w_full()
                    .h(px(36.0))
                    .rounded(px(10.0))
                    .bg(ACCENT)
                    .hover(|s| s.bg(ACCENT_HOVER))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(BG_EDITOR)
                    .text_size(px(14.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("＋  新建便签")
                    .on_click(cx.listener(|this, _, window, cx| this.new_note(window, cx))),
            )
            // 搜索
            .child(div().text_size(px(14.0)).child(Input::new(&self.search_input)))
            // 分组标题
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(TEXT_SOFT)
                            .child("全部便签"),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(TEXT_FAINT)
                            .child(format!("{total} 张")),
                    ),
            )
            // 卡片列表（独立滚动）
            .child(
                div()
                    .id("card-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(self.render_cards(cx)),
            )
            .into_any_element()
    }

    fn render_cards(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search_input.read(cx).value().to_string();
        let ids = self.notes.sorted_ids(&query);

        if ids.is_empty() {
            return div()
                .pt(px(36.0))
                .flex()
                .justify_center()
                .text_size(px(14.0))
                .text_color(TEXT_SOFT)
                .child(if query.trim().is_empty() {
                    "还没有便签"
                } else {
                    "没有匹配的便签"
                })
                .into_any_element();
        }

        let mut list = div().flex().flex_col().gap(px(7.0)).pb(px(8.0));
        for id in ids {
            let Some(note) = self.notes.get(id) else {
                continue;
            };
            let title = truncate(&note.display_title(), 20);
            let base = theme::color_for(note.color);
            let selected = self.current == Some(id);
            let idle_bg = if selected { base } else { theme::dimmed(base) };

            let card = div()
                .id(("card", id as usize))
                .relative()
                .flex_none()
                .w_full()
                .h(px(CARD_HEIGHT))
                .px(px(16.0))
                .rounded(px(12.0))
                .bg(idle_bg)
                .border_1()
                .border_color(if selected { ACCENT } else { CARD_BORDER })
                .when(selected, |s| s.shadow_md())
                .when(!selected, |s| s.shadow_sm())
                .hover(move |s| s.bg(base).shadow_md())
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |s| {
                    s.child(
                        div()
                            .absolute()
                            .left(px(5.0))
                            .top(px(15.0))
                            .bottom(px(15.0))
                            .w(px(3.0))
                            .rounded(px(1.5))
                            .bg(ACCENT),
                    )
                })
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(16.0))
                        .text_color(TEXT_DARK)
                        .child(title),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.select(id, window, cx)))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                        this.card_menu = Some((id, ev.position));
                        cx.notify();
                    }),
                );
            list = list.child(card);
        }
        list.into_any_element()
    }

    /// 卡片右键菜单：浮在最上层，点菜单外任意位置关闭。
    pub(crate) fn render_card_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (id, position) = self.card_menu?;
        Some(
            deferred(
                anchored().position(position).child(
                    div()
                        .id("card-menu")
                        .occlude()
                        .min_w(px(112.0))
                        .p(px(4.0))
                        .rounded(px(8.0))
                        .bg(BG_EDITOR)
                        .border_1()
                        .border_color(DIVIDER)
                        .shadow_lg()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.card_menu = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .id("card-menu-delete")
                                .px(px(10.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .text_size(px(13.0))
                                .text_color(ERROR_RED)
                                .hover(|s| s.bg(ERROR_SOFT))
                                .cursor_pointer()
                                .child("删除便签")
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.request_delete(id, cx)),
                                ),
                        ),
                ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
