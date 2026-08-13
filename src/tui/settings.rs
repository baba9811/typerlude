use super::{
    format::{focus_marker, language_name, toggle_name},
    theme::ThemeStyles,
    titled,
};
use crate::{
    app::App,
    i18n::{TextKey, text},
    model::Language,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph},
};

pub(super) fn render_settings(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeSettings), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (
        practice_language,
        ui_language,
        keyboard,
        finger_guide,
        live_speed,
        accuracy,
        adaptive,
        updates,
    ) = match language {
        Language::Ko => (
            "연습 언어",
            "화면 언어",
            "키보드",
            "손가락 안내",
            "실시간 속도",
            "정확도 표시",
            "적응형",
            "업데이트 확인",
        ),
        Language::En => (
            "Language",
            "UI language",
            "keyboard",
            "finger guide",
            "live speed",
            "accuracy",
            "adaptive",
            "updates",
        ),
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                "{}{practice_language}: {}",
                focus_marker(app, 0),
                language_name(app.settings.language)
            )),
            Line::from(format!(
                "{}{ui_language}: {}",
                focus_marker(app, 1),
                language_name(app.settings.ui_language)
            )),
            Line::from(format!(
                "{}{}: {}",
                focus_marker(app, 2),
                text(language, TextKey::Theme),
                app.settings.theme
            )),
            Line::from(format!(
                "{}{keyboard}: {}",
                focus_marker(app, 3),
                toggle_name(language, app.settings.show_keyboard)
            )),
            Line::from(format!(
                "{}{finger_guide}: {}",
                focus_marker(app, 4),
                toggle_name(language, app.settings.show_finger_guide)
            )),
            Line::from(format!(
                "{}{live_speed}: {}",
                focus_marker(app, 5),
                toggle_name(language, app.settings.show_live_speed)
            )),
            Line::from(format!(
                "{}{accuracy}: {}",
                focus_marker(app, 6),
                toggle_name(language, app.settings.show_accuracy)
            )),
            Line::from(format!(
                "{}{adaptive}: {}",
                focus_marker(app, 7),
                toggle_name(language, app.settings.adaptive)
            )),
            Line::from(format!(
                "{}{updates}: {}",
                focus_marker(app, 8),
                toggle_name(language, app.settings.check_updates)
            )),
        ])
        .style(styles.base),
        inner,
    );
}

pub(super) fn render_themes(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Theme), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = usize::from(inner.height).max(1);
    let start = app.focus().saturating_sub(visible - 1);
    frame.render_widget(
        List::new(
            app.themes
                .ids()
                .enumerate()
                .skip(start)
                .take(visible)
                .map(|(index, id)| {
                    let marker = if app.focus() == index { "> " } else { "  " };
                    let selected = if id == app.settings.theme { "*" } else { " " };
                    ListItem::new(format!("{marker}{selected} {id}"))
                }),
        )
        .style(styles.base),
        inner,
    );
}

pub(super) fn render_help(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Help), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = match language {
        Language::Ko => vec![
            Line::from("이동: Tab / Shift+Tab / ↑↓ / j / k"),
            Line::from("선택/편집: Enter / ←→ · 콘텐츠 비활성화: 상세에서 d 두 번"),
            Line::from("뒤로/홈에서 종료: Esc / q / ㅂ · 즉시 종료: Ctrl+C · 도움말: ?"),
            Line::from("시험 외: Esc / Ctrl+P 일시 정지 · 정지 중 q 또는 ㅂ을 두 번 눌러 나가기"),
            Line::from("시험: Esc 나가기 확인 열기/취소 · q 또는 ㅂ으로 확인"),
            Line::from("결과 r: 같은 대상/설정 · n: 빠른/단어/문장/카탈로그 긴 글만"),
            Line::from("업데이트 알림: l 나중에 · s 이번 버전 건너뛰기"),
        ],
        Language::En => vec![
            Line::from("Move: Tab / Shift+Tab / ↑↓ / j / k"),
            Line::from("Select/Edit: Enter / ←→ · Content Disable: d twice in detail"),
            Line::from("Back / quit from Home: Esc / q / ㅂ · Quit now: Ctrl+C · Help: ?"),
            Line::from("Non-Test: Esc / Ctrl+P pause · paused: press q or ㅂ twice to leave"),
            Line::from("Test: Esc opens/cancels leave · q or ㅂ confirms"),
            Line::from(
                "Result r: exact target/options · n: Quick/Words/Sentence/catalog Long only",
            ),
            Line::from("Update notice: l later · s skip this version"),
        ],
    };
    lines.extend([
        Line::from(""),
        Line::from("typerlude quick|keys|words|sentence|long|test"),
        Line::from("typerlude stats|history|themes"),
        Line::from("typerlude content list"),
        Line::from("typerlude content add PACK.toml"),
        Line::from("typerlude content validate [PACK.toml]"),
        Line::from("typerlude content disable PACK_ID"),
        Line::from("typerlude paths|licenses|update"),
        Line::from("typerlude --help|--version|--smoke"),
        Line::from("typerlude FILE | typerlude practice FILE | cat FILE | typerlude"),
    ]);
    frame.render_widget(Paragraph::new(lines).style(styles.base), inner);
}
