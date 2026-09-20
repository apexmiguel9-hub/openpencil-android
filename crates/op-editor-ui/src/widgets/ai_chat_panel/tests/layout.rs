//! Layout + `from_editor` snapshot tests for the AI chat panel.

#[allow(unused_imports)]
use super::super::tests_paint::{assert_close, color_close, rect_close};
use super::super::*;

#[test]
fn layout_reports_fixed_size() {
    let s = EditorState::new();
    let p = AIChatPlaceholder::from_editor(&s);
    let cx = LayoutCx {
        available_width: 9999.0,
        dpi: 1.0,
    };
    let lb = p.layout(&cx);
    assert_eq!(lb.rect.size.x, AI_CHAT_WIDTH);
    assert_eq!(lb.rect.size.y, AI_CHAT_HEIGHT);
}

#[test]
fn examples_grid_has_four_cards() {
    assert_eq!(example_cards(op_editor_core::Locale::EnUs).len(), 4);
}

#[test]
fn second_example_uses_short_title_and_full_music_prompt() {
    let en = example_cards(op_editor_core::Locale::EnUs);
    assert_eq!(en[1].title, "Dark music streaming mobile app");
    assert_eq!(
        en[1].prompt,
        "Design a dark-themed music streaming mobile app home screen. Include a greeting \"Good evening\", horizontal scrollable \"Recently Played\" album art cards, \"Made For You\" section with 3 playlist cards showing cover art and playlist names, \"New Releases\" section with 4 album cards in a 2x2 grid, and a floating mini player bar at the bottom showing current track with play/pause controls. Bottom tab bar (Home, Search, Library, Premium). Dark background with lime green accent."
    );

    let zh = example_cards(op_editor_core::Locale::ZhCn);
    assert_eq!(zh[1].title, "暗色音乐流媒体 App 首页");
    assert_eq!(
        zh[1].prompt,
        "设计一个暗色音乐流媒体App首页。包含问候语\"晚上好\"、\"最近播放\"横向滑动专辑封面卡片、\"为你推荐\"区3张歌单卡片（封面和歌单名）、\"新发行\"区4张专辑卡片2x2网格、底部悬浮迷你播放器（当前曲目+播放/暂停控件）。底部导航栏（首页、搜索、音乐库、会员）。深色背景搭配荧光绿强调。"
    );
}

#[test]
fn from_editor_tracks_selection_count_for_toolbar() {
    let mut s = EditorState::new();
    s.selection.set = vec![
        op_editor_core::NodeId::new("n1"),
        op_editor_core::NodeId::new("n2"),
    ];
    let panel = AIChatPlaceholder::from_editor(&s);

    assert_eq!(panel.selected_count, 2);
}

#[test]
fn from_editor_uses_try_example_hint() {
    // old→new (#33 restyle): empty-state header uses "ai.tryExample" key
    // ("Try an example to design…") instead of the old "ai.startDesigning".
    let s = EditorState::new();
    let panel = AIChatPlaceholder::from_editor(&s);

    assert_eq!(
        panel.label_start_with_ai,
        op_i18n::translate(s.editor_ui.locale, "ai.tryExample")
    );
}
