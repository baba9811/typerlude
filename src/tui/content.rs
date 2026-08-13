use super::{format::language_name, no_data, theme::ThemeStyles, titled};
use crate::{
    app::App,
    content::ContentKind,
    i18n::{TextKey, text},
    model::Language,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{List, ListItem, Paragraph, Wrap},
};

pub(super) fn render_content(frame: &mut Frame<'_>, app: &App, area: Rect, styles: ThemeStyles) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::HomeContent), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let packs = app.content_packs();
    if packs.is_empty() {
        frame.render_widget(no_data(language, styles), inner);
        return;
    }
    let (enabled, disabled, built_in, user) = match language {
        Language::Ko => ("활성", "비활성", "내장", "사용자"),
        Language::En => ("enabled", "disabled", "built-in", "user"),
    };
    let visible = usize::from(inner.height / 2).max(1);
    let start = app.focus().saturating_sub(visible - 1);
    frame.render_widget(
        List::new(
            packs
                .iter()
                .enumerate()
                .skip(start)
                .take(visible)
                .map(|(index, pack)| {
                    let mut kinds = Vec::new();
                    for &kind in &pack.kinds {
                        let name = content_kind_name(language, kind);
                        if !kinds.contains(&name) {
                            kinds.push(name);
                        }
                    }
                    let kinds = kinds.join(",");
                    ListItem::new(vec![
                        Line::from(format!(
                            "{}{} · {} · {} · {} · {}",
                            if app.focus() == index { "> " } else { "  " },
                            pack.id,
                            if pack.enabled { enabled } else { disabled },
                            if pack.built_in { built_in } else { user },
                            language_name(pack.language),
                            pack.items,
                        )),
                        Line::from(format!(
                            "    {} · {} · {}",
                            pack.licenses.join(","),
                            kinds,
                            pack.sample_item_id,
                        )),
                    ])
                }),
        )
        .style(styles.base),
        inner,
    );
}

pub(super) fn render_content_detail(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    styles: ThemeStyles,
) {
    let language = app.settings.ui_language;
    let block = titled(text(language, TextKey::Sources), styles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(summary) = app.content_detail_pack() else {
        frame.render_widget(no_data(language, styles), inner);
        return;
    };
    let pack_id_value = &summary.id;
    let (
        item_id,
        pack_id,
        author,
        source_id,
        source_url,
        license,
        license_url,
        retrieved,
        modified,
        yes,
        no,
    ) = match language {
        Language::Ko => (
            "항목 ID",
            "팩 ID",
            "저자",
            "출처 ID",
            "출처 URL",
            "라이선스",
            "라이선스 URL",
            "확인일",
            "수정됨",
            "예",
            "아니요",
        ),
        Language::En => (
            "item ID",
            "pack ID",
            "author",
            "source ID",
            "source URL",
            "license",
            "license URL",
            "retrieved",
            "modified",
            "yes",
            "no",
        ),
    };
    let mut lines = vec![Line::from(format!(
        "{pack_id}: {pack_id_value} · {}: {} · {}: {} · {}",
        match language {
            Language::Ko => "언어",
            Language::En => "language",
        },
        language_name(summary.language),
        match language {
            Language::Ko => "항목 수",
            Language::En => "items",
        },
        summary.items,
        match (language, summary.enabled) {
            (Language::Ko, true) => "활성",
            (Language::Ko, false) => "비활성",
            (Language::En, true) => "enabled",
            (Language::En, false) => "disabled",
        }
    ))];
    if let Some(provenance) = summary.provenance.get(app.focus()) {
        let scope = if let Some(sample_item_id) = &provenance.item_id {
            format!("{item_id}: {sample_item_id}")
        } else {
            match language {
                Language::Ko => "범위: 팩",
                Language::En => "scope: pack",
            }
            .to_owned()
        };
        lines.push(Line::from(format!(
            "{} {}/{} · {scope} · ↑/↓",
            match language {
                Language::Ko => "출처",
                Language::En => "Provenance",
            },
            app.focus() + 1,
            summary.provenance.len()
        )));
        let source = &provenance.source;
        lines.extend([
            Line::from(format!("{author}: {}", source.author)),
            Line::from(format!("{source_id}: {}", source.source_id)),
            Line::from(format!("{source_url}: {}", source.source_url)),
            Line::from(format!("{license}: {}", source.license)),
            Line::from(format!("{license_url}: {}", source.license_url)),
            Line::from(format!(
                "{retrieved}: {} · {modified}: {}",
                source.retrieved_at,
                if source.modified { yes } else { no }
            )),
        ]);
    }
    lines.extend([
        Line::from("typerlude content add PACK.toml · typerlude content validate PACK.toml"),
        Line::from("typerlude content disable PACK_ID · typerlude licenses"),
    ]);
    lines.push(Line::from(if summary.built_in {
        match language {
            Language::Ko => "내장 팩은 비활성화할 수 없습니다",
            Language::En => "Built-in packs cannot be disabled",
        }
    } else if !summary.enabled {
        match language {
            Language::Ko => "사용자 팩이 비활성화됨",
            Language::En => "User pack is disabled",
        }
    } else if app.content_disable_confirmation() {
        match language {
            Language::Ko => "확인하려면 d를 다시 누르세요",
            Language::En => "Press d again to confirm",
        }
    } else {
        match language {
            Language::Ko => "d: 비활성화",
            Language::En => "d: Disable",
        }
    }));
    frame.render_widget(
        Paragraph::new(lines)
            .style(styles.base)
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn content_kind_name(language: Language, kind: ContentKind) -> &'static str {
    text(
        language,
        match kind {
            ContentKind::Word => TextKey::HomeWords,
            ContentKind::Sentence | ContentKind::Quote => TextKey::HomeSentence,
            ContentKind::Text => TextKey::HomeLong,
        },
    )
}
