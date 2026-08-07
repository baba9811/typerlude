mod hangul;

use crate::model::Language;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

pub fn normalize_nfc(text: &str) -> String {
    text.nfc().collect()
}

pub fn split_graphemes(text: &str) -> Vec<String> {
    normalize_nfc(text)
        .graphemes(true)
        .map(str::to_owned)
        .collect()
}

pub fn key_units(language: Language, text: &str) -> Vec<char> {
    let normalized = normalize_nfc(text);
    normalized
        .chars()
        .flat_map(|ch| match language {
            Language::Ko => hangul::syllable_units(ch)
                .or_else(|| hangul::compatibility_units(ch))
                .unwrap_or_else(|| vec![ch]),
            Language::En => vec![if ch.is_ascii_alphabetic() {
                ch.to_ascii_lowercase()
            } else {
                ch
            }],
        })
        .collect()
}

pub fn unit_count(language: Language, text: &str) -> u64 {
    key_units(language, text).len() as u64
}

#[cfg(test)]
mod tests {
    use super::{key_units, normalize_nfc, split_graphemes, unit_count};
    use crate::model::Language;

    #[test]
    fn nfc_and_nfd_compare_as_the_same_grapheme() {
        assert_eq!(normalize_nfc("한"), normalize_nfc("한"));
        assert_eq!(split_graphemes("한글"), vec!["한", "글"]);
    }

    #[test]
    fn korean_units_expand_compounds_without_counting_shift() {
        assert_eq!(key_units(Language::Ko, "한"), vec!['ㅎ', 'ㅏ', 'ㄴ']);
        assert_eq!(key_units(Language::Ko, "과"), vec!['ㄱ', 'ㅗ', 'ㅏ']);
        assert_eq!(key_units(Language::Ko, "값"), vec!['ㄱ', 'ㅏ', 'ㅂ', 'ㅅ']);
        assert_eq!(key_units(Language::Ko, "까"), vec!['ㄲ', 'ㅏ']);
    }

    #[test]
    fn english_units_are_physical_character_keys() {
        assert_eq!(key_units(Language::En, "A !"), vec!['a', ' ', '!']);
        assert_eq!(unit_count(Language::En, "A !"), 3);
    }

    #[test]
    fn every_modern_hangul_syllable_has_two_to_five_units() {
        for value in 0xAC00..=0xD7A3 {
            let ch = char::from_u32(value).unwrap();
            let count = key_units(Language::Ko, &ch.to_string()).len();
            assert!((2..=5).contains(&count), "{ch}: {count}");
        }
    }
}
