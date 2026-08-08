use crate::norm::SYMBOLS;
use once_cell::sync::Lazy;
use std::collections::HashMap;

static SYMBOL_TO_ID: Lazy<HashMap<String, i32>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for (i, symbols) in SYMBOLS.iter().enumerate() {
        map.insert(symbols.to_string(), i as i32);
    }
    map
});

pub fn cleaned_text_to_sequence(
    cleaned_phones: Vec<String>,
    tones: Vec<i32>,
) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    let phones: Vec<i64> = cleaned_phones
        .iter()
        .map(|phone| *SYMBOL_TO_ID.get(phone).unwrap() as i64)
        .collect();
    let tones: Vec<i64> = tones.iter().map(|tone| (*tone + 6) as i64).collect();
    let lang_ids: Vec<i64> = vec![1; phones.len()];
    (phones, tones, lang_ids)
}
