use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Mora {
    pub mora: String,
    pub consonant: Option<String>,
    pub vowel: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoraFile {
    pub minimum: Vec<Mora>,
    pub additional: Vec<Mora>,
}

static MORA_LIST_MINIMUM: Lazy<Vec<Mora>> = Lazy::new(|| {
    let data: MoraFile = serde_json::from_str(include_str!("./mora_list.json")).unwrap();
    data.minimum
});

static MORA_LIST_ADDITIONAL: Lazy<Vec<Mora>> = Lazy::new(|| {
    let data: MoraFile = serde_json::from_str(include_str!("./mora_list.json")).unwrap();
    data.additional
});

pub static MORA_PHONEMES_TO_MORA_KATA: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for mora in MORA_LIST_MINIMUM.iter() {
        map.insert(
            format!(
                "{}{}",
                mora.consonant.clone().unwrap_or("".to_string()),
                mora.vowel
            ),
            mora.mora.clone(),
        );
    }
    map
});

pub static MORA_KATA_TO_MORA_PHONEMES: Lazy<HashMap<String, (Option<String>, String)>> =
    Lazy::new(|| {
        let mut map = HashMap::new();
        for mora in MORA_LIST_MINIMUM.iter().chain(MORA_LIST_ADDITIONAL.iter()) {
            map.insert(
                mora.mora.clone(),
                (mora.consonant.clone(), mora.vowel.clone()),
            );
        }
        map
    });

pub static CONSONANTS: Lazy<Vec<String>> = Lazy::new(|| {
    let consonants = MORA_KATA_TO_MORA_PHONEMES
        .values()
        .filter_map(|(consonant, _)| consonant.clone())
        .collect::<Vec<_>>();
    consonants
});

pub const VOWELS: [&str; 6] = ["a", "i", "u", "e", "o", "N"];
