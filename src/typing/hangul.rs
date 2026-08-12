const S_BASE: u32 = 0xAC00;
const S_COUNT: u32 = 19 * 21 * 28;

const INITIAL: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

const MEDIAL: [&[char]; 21] = [
    &['ㅏ'],
    &['ㅐ'],
    &['ㅑ'],
    &['ㅒ'],
    &['ㅓ'],
    &['ㅔ'],
    &['ㅕ'],
    &['ㅖ'],
    &['ㅗ'],
    &['ㅗ', 'ㅏ'],
    &['ㅗ', 'ㅐ'],
    &['ㅗ', 'ㅣ'],
    &['ㅛ'],
    &['ㅜ'],
    &['ㅜ', 'ㅓ'],
    &['ㅜ', 'ㅔ'],
    &['ㅜ', 'ㅣ'],
    &['ㅠ'],
    &['ㅡ'],
    &['ㅡ', 'ㅣ'],
    &['ㅣ'],
];

const FINAL: [&[char]; 28] = [
    &[],
    &['ㄱ'],
    &['ㄲ'],
    &['ㄱ', 'ㅅ'],
    &['ㄴ'],
    &['ㄴ', 'ㅈ'],
    &['ㄴ', 'ㅎ'],
    &['ㄷ'],
    &['ㄹ'],
    &['ㄹ', 'ㄱ'],
    &['ㄹ', 'ㅁ'],
    &['ㄹ', 'ㅂ'],
    &['ㄹ', 'ㅅ'],
    &['ㄹ', 'ㅌ'],
    &['ㄹ', 'ㅍ'],
    &['ㄹ', 'ㅎ'],
    &['ㅁ'],
    &['ㅂ'],
    &['ㅂ', 'ㅅ'],
    &['ㅅ'],
    &['ㅆ'],
    &['ㅇ'],
    &['ㅈ'],
    &['ㅊ'],
    &['ㅋ'],
    &['ㅌ'],
    &['ㅍ'],
    &['ㅎ'],
];

pub(super) fn is_supported(ch: char) -> bool {
    syllable_units(ch).is_some() || compatibility_units(ch).is_some()
}

pub fn syllable_units(ch: char) -> Option<Vec<char>> {
    let index = (ch as u32).checked_sub(S_BASE)?;
    if index >= S_COUNT {
        return None;
    }
    let initial = (index / (21 * 28)) as usize;
    let medial = ((index % (21 * 28)) / 28) as usize;
    let final_index = (index % 28) as usize;
    let mut units = vec![INITIAL[initial]];
    units.extend_from_slice(MEDIAL[medial]);
    units.extend_from_slice(FINAL[final_index]);
    Some(units)
}

pub fn compatibility_units(ch: char) -> Option<Vec<char>> {
    Some(match ch {
        'ㅘ' => vec!['ㅗ', 'ㅏ'],
        'ㅙ' => vec!['ㅗ', 'ㅐ'],
        'ㅚ' => vec!['ㅗ', 'ㅣ'],
        'ㅝ' => vec!['ㅜ', 'ㅓ'],
        'ㅞ' => vec!['ㅜ', 'ㅔ'],
        'ㅟ' => vec!['ㅜ', 'ㅣ'],
        'ㅢ' => vec!['ㅡ', 'ㅣ'],
        'ㄳ' => vec!['ㄱ', 'ㅅ'],
        'ㄵ' => vec!['ㄴ', 'ㅈ'],
        'ㄶ' => vec!['ㄴ', 'ㅎ'],
        'ㄺ' => vec!['ㄹ', 'ㄱ'],
        'ㄻ' => vec!['ㄹ', 'ㅁ'],
        'ㄼ' => vec!['ㄹ', 'ㅂ'],
        'ㄽ' => vec!['ㄹ', 'ㅅ'],
        'ㄾ' => vec!['ㄹ', 'ㅌ'],
        'ㄿ' => vec!['ㄹ', 'ㅍ'],
        'ㅀ' => vec!['ㄹ', 'ㅎ'],
        'ㅄ' => vec!['ㅂ', 'ㅅ'],
        'ㄱ' | 'ㄲ' | 'ㄴ' | 'ㄷ' | 'ㄸ' | 'ㄹ' | 'ㅁ' | 'ㅂ' | 'ㅃ' | 'ㅅ' | 'ㅆ' | 'ㅇ'
        | 'ㅈ' | 'ㅉ' | 'ㅊ' | 'ㅋ' | 'ㅌ' | 'ㅍ' | 'ㅎ' | 'ㅏ' | 'ㅐ' | 'ㅑ' | 'ㅒ' | 'ㅓ'
        | 'ㅔ' | 'ㅕ' | 'ㅖ' | 'ㅗ' | 'ㅛ' | 'ㅜ' | 'ㅠ' | 'ㅡ' | 'ㅣ' => vec![ch],
        _ => return None,
    })
}
