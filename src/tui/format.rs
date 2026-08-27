use crate::{
    app::App,
    diagnostic::terminal_safe,
    i18n::{TextKey, text},
    model::{Language, PracticeKind},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) fn terminal_line(value: &str, width: u16) -> String {
    let value = terminal_safe(value);
    let width = usize::from(width);
    if UnicodeWidthStr::width(value.as_str()) <= width {
        return value;
    }
    if width == 0 {
        return String::new();
    }

    let mut line = String::new();
    let available = width - UnicodeWidthStr::width("…");
    for grapheme in value.graphemes(true) {
        if UnicodeWidthStr::width(line.as_str()) + UnicodeWidthStr::width(grapheme) > available {
            break;
        }
        line.push_str(grapheme);
    }
    line.push('…');
    line
}

pub(super) fn focus_marker(app: &App, index: usize) -> &'static str {
    if app.focus() == index { "> " } else { "  " }
}

pub(super) const fn language_name(language: Language) -> &'static str {
    match language {
        Language::Ko => "ko",
        Language::En => "en",
    }
}

pub(super) const fn toggle_name(language: Language, enabled: bool) -> &'static str {
    match (language, enabled) {
        (Language::Ko, true) => "켜짐",
        (Language::Ko, false) => "꺼짐",
        (Language::En, true) => "true",
        (Language::En, false) => "false",
    }
}

pub(super) fn practice_name(language: Language, kind: PracticeKind) -> &'static str {
    text(
        language,
        match kind {
            PracticeKind::Quick => TextKey::HomeQuick,
            PracticeKind::Key => TextKey::HomeKeys,
            PracticeKind::Words => TextKey::HomeWords,
            PracticeKind::Sentence => TextKey::HomeSentence,
            PracticeKind::Long => TextKey::HomeLong,
            PracticeKind::Test => TextKey::HomeTest,
        },
    )
}

pub(super) fn speed_values(ui_language: Language, kpm: f64, wpm: f64) -> String {
    match ui_language {
        Language::Ko => format!("타수 {kpm:.1} 타/분 · WPM {wpm:.1}"),
        Language::En => format!("KPM {kpm:.1} · WPM {wpm:.1}"),
    }
}

pub(super) fn grouped_u64(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len().saturating_sub(1) / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::{grouped_u64, speed_values, terminal_line};
    use crate::model::Language;

    #[test]
    fn speed_value_helpers_localize_both_units() {
        assert_eq!(
            speed_values(Language::En, 200.0, 40.0),
            "KPM 200.0 · WPM 40.0"
        );
        assert_eq!(
            speed_values(Language::Ko, 200.0, 40.0),
            "타수 200.0 타/분 · WPM 40.0"
        );
    }

    #[test]
    fn scores_use_thousands_separators() {
        for (score, expected) in [
            (0, "0"),
            (999, "999"),
            (1_000, "1,000"),
            (1_000_000, "1,000,000"),
            (u64::MAX, "18,446,744,073,709,551,615"),
        ] {
            assert_eq!(grouped_u64(score), expected);
        }
    }

    #[test]
    fn terminal_line_truncates_wide_graphemes_at_zero_and_tiny_widths() {
        assert_eq!(terminal_line("界", 0), "");
        assert_eq!(terminal_line("界", 1), "…");
        assert_eq!(terminal_line("界界", 2), "…");
        assert_eq!(terminal_line("界界", 3), "界…");
    }
}
