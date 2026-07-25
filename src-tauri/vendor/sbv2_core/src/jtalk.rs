/*
このファイルのコードは
https://github.com/litagin02/Style-Bert-VITS2/blob/master/style_bert_vits2/nlp/japanese/g2p.py
を参考にRustに書き換えています。

以下はライセンスです。
                   GNU LESSER GENERAL PUBLIC LICENSE
                       Version 3, 29 June 2007

 Copyright (C) 2007 Free Software Foundation, Inc. <https://fsf.org/>
 Everyone is permitted to copy and distribute verbatim copies
 of this license document, but changing it is not allowed.


  This version of the GNU Lesser General Public License incorporates
the terms and conditions of version 3 of the GNU General Public
License, supplemented by the additional permissions listed below.

  0. Additional Definitions.

  As used herein, "this License" refers to version 3 of the GNU Lesser
General Public License, and the "GNU GPL" refers to version 3 of the GNU
General Public License.

  "The Library" refers to a covered work governed by this License,
other than an Application or a Combined Work as defined below.

  An "Application" is any work that makes use of an interface provided
by the Library, but which is not otherwise based on the Library.
Defining a subclass of a class defined by the Library is deemed a mode
of using an interface provided by the Library.

  A "Combined Work" is a work produced by combining or linking an
Application with the Library.  The particular version of the Library
with which the Combined Work was made is also called the "Linked
Version".

  The "Minimal Corresponding Source" for a Combined Work means the
Corresponding Source for the Combined Work, excluding any source code
for portions of the Combined Work that, considered in isolation, are
based on the Application, and not on the Linked Version.

  The "Corresponding Application Code" for a Combined Work means the
object code and/or source code for the Application, including any data
and utility programs needed for reproducing the Combined Work from the
Application, but excluding the System Libraries of the Combined Work.

  1. Exception to Section 3 of the GNU GPL.

  You may convey a covered work under sections 3 and 4 of this License
without being bound by section 3 of the GNU GPL.

  2. Conveying Modified Versions.

  If you modify a copy of the Library, and, in your modifications, a
facility refers to a function or data to be supplied by an Application
that uses the facility (other than as an argument passed when the
facility is invoked), then you may convey a copy of the modified
version:

   a) under this License, provided that you make a good faith effort to
   ensure that, in the event an Application does not supply the
   function or data, the facility still operates, and performs
   whatever part of its purpose remains meaningful, or

   b) under the GNU GPL, with none of the additional permissions of
   this License applicable to that copy.

  3. Object Code Incorporating Material from Library Header Files.

  The object code form of an Application may incorporate material from
a header file that is part of the Library.  You may convey such object
code under terms of your choice, provided that, if the incorporated
material is not limited to numerical parameters, data structure
layouts and accessors, or small macros, inline functions and templates
(ten or fewer lines in length), you do both of the following:

   a) Give prominent notice with each copy of the object code that the
   Library is used in it and that the Library and its use are
   covered by this License.

   b) Accompany the object code with a copy of the GNU GPL and this license
   document.

  4. Combined Works.

  You may convey a Combined Work under terms of your choice that,
taken together, effectively do not restrict modification of the
portions of the Library contained in the Combined Work and reverse
engineering for debugging such modifications, if you also do each of
the following:

   a) Give prominent notice with each copy of the Combined Work that
   the Library is used in it and that the Library and its use are
   covered by this License.

   b) Accompany the Combined Work with a copy of the GNU GPL and this license
   document.

   c) For a Combined Work that displays copyright notices during
   execution, include the copyright notice for the Library among
   these notices, as well as a reference directing the user to the
   copies of the GNU GPL and this license document.

   d) Do one of the following:

       0) Convey the Minimal Corresponding Source under the terms of this
       License, and the Corresponding Application Code in a form
       suitable for, and under terms that permit, the user to
       recombine or relink the Application with a modified version of
       the Linked Version to produce a modified Combined Work, in the
       manner specified by section 6 of the GNU GPL for conveying
       Corresponding Source.

       1) Use a suitable shared library mechanism for linking with the
       Library.  A suitable mechanism is one that (a) uses at run time
       a copy of the Library already present on the user's computer
       system, and (b) will operate properly with a modified version
       of the Library that is interface-compatible with the Linked
       Version.

   e) Provide Installation Information, but only if you would otherwise
   be required to provide such information under section 6 of the
   GNU GPL, and only to the extent that such information is
   necessary to install and execute a modified version of the
   Combined Work produced by recombining or relinking the
   Application with a modified version of the Linked Version. (If
   you use option 4d0, the Installation Information must accompany
   the Minimal Corresponding Source and Corresponding Application
   Code. If you use option 4d1, you must provide the Installation
   Information in the manner specified by section 6 of the GNU GPL
   for conveying Corresponding Source.)

  5. Combined Libraries.

  You may place library facilities that are a work based on the
Library side by side in a single library together with other library
facilities that are not Applications and are not covered by this
License, and convey such a combined library under terms of your
choice, if you do both of the following:

   a) Accompany the combined library with a copy of the same work based
   on the Library, uncombined with any other library facilities,
   conveyed under the terms of this License.

   b) Give prominent notice with the combined library that part of it
   is a work based on the Library, and explaining where to find the
   accompanying uncombined form of the same work.

  6. Revised Versions of the GNU Lesser General Public License.

  The Free Software Foundation may publish revised and/or new versions
of the GNU Lesser General Public License from time to time. Such new
versions will be similar in spirit to the present version, but may
differ in detail to address new problems or concerns.

  Each version is given a distinguishing version number. If the
Library as you received it specifies that a certain numbered version
of the GNU Lesser General Public License "or any later version"
applies to it, you have the option of following the terms and
conditions either of that published version or of any later version
published by the Free Software Foundation. If the Library as you
received it does not specify a version number of the GNU Lesser
General Public License, you may choose any version of the GNU Lesser
General Public License ever published by the Free Software Foundation.

  If the Library as you received it specifies that a proxy can decide
whether future versions of the GNU Lesser General Public License shall
apply, that proxy's public statement of acceptance of any version is
permanent authorization for you to choose that version for the
Library.
*/
use crate::error::{Error, Result};
use crate::mora::{CONSONANTS, MORA_KATA_TO_MORA_PHONEMES, MORA_PHONEMES_TO_MORA_KATA, VOWELS};
use crate::norm::{replace_punctuation, PUNCTUATIONS};
use jpreprocess::{kind, DefaultTokenizer, JPreprocess, SystemDictionaryConfig, UserDictionary};
use once_cell::sync::Lazy;
use regex::Regex;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::sync::Arc;

type JPreprocessType = JPreprocess<DefaultTokenizer>;

#[cfg(feature = "agpl_dict")]
fn agpl_dict() -> Result<Option<UserDictionary>> {
    Ok(Some(
        UserDictionary::load(include_bytes!(concat!(env!("OUT_DIR"), "/all.bin")))
            .map_err(|e| Error::LinderaError(e.to_string()))?,
    ))
}

#[cfg(not(feature = "agpl_dict"))]
fn agpl_dict() -> Result<Option<UserDictionary>> {
    Ok(None)
}

fn initialize_jtalk() -> Result<JPreprocessType> {
    let sdic =
        SystemDictionaryConfig::Bundled(kind::JPreprocessDictionaryKind::NaistJdic).load()?;
    let jpreprocess = JPreprocess::with_dictionaries(sdic, agpl_dict()?);
    Ok(jpreprocess)
}

macro_rules! hash_set {
    ($($elem:expr),* $(,)?) => {{
        let mut set = HashSet::new();
        $(
            set.insert($elem);
        )*
        set
    }};
}

pub struct JTalk {
    pub jpreprocess: Arc<JPreprocessType>,
}

impl JTalk {
    pub fn new() -> Result<Self> {
        let jpreprocess = Arc::new(initialize_jtalk()?);
        Ok(Self { jpreprocess })
    }

    pub fn num2word(&self, text: &str) -> Result<String> {
        let mut parsed = self.jpreprocess.text_to_njd(text)?;
        parsed.preprocess();
        let texts: Vec<String> = parsed
            .nodes
            .iter()
            .map(|x| x.get_string().to_string())
            .collect();
        Ok(texts.join(""))
    }

    pub fn process_text(&self, text: &str) -> Result<JTalkProcess> {
        let parsed = self.jpreprocess.run_frontend(text)?;
        let jtalk_process = JTalkProcess::new(Arc::clone(&self.jpreprocess), parsed);
        Ok(jtalk_process)
    }
}

static KATAKANA_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\u30A0-\u30FF]+").unwrap());
static MORA_PATTERN: Lazy<Vec<String>> = Lazy::new(|| {
    let mut sorted_keys: Vec<String> = MORA_KATA_TO_MORA_PHONEMES.keys().cloned().collect();
    sorted_keys.sort_by_key(|b| Reverse(b.len()));
    sorted_keys
});
static LONG_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\w)(ー*)").unwrap());

fn phone_tone_to_kana(phones: Vec<String>, tones: Vec<i32>) -> Vec<(String, i32)> {
    let phones = &phones[1..];
    let tones = &tones[1..];
    let mut results = Vec::new();
    let mut current_mora = String::new();
    for ((phone, _next_phone), (&tone, &next_tone)) in phones
        .iter()
        .zip(phones.iter().skip(1))
        .zip(tones.iter().zip(tones.iter().skip(1)))
    {
        if PUNCTUATIONS.contains(&phone.clone().as_str()) {
            results.push((phone.to_string(), tone));
            continue;
        }
        if CONSONANTS.contains(&phone.clone()) {
            assert_eq!(current_mora, "");
            assert_eq!(tone, next_tone);
            current_mora = phone.to_string()
        } else {
            current_mora += phone;
            let kana = MORA_PHONEMES_TO_MORA_KATA.get(&current_mora).unwrap();
            results.push((kana.to_string(), tone));
            current_mora = String::new();
        }
    }
    results
}

pub struct JTalkProcess {
    jpreprocess: Arc<JPreprocessType>,
    parsed: Vec<String>,
}

impl JTalkProcess {
    fn new(jpreprocess: Arc<JPreprocessType>, parsed: Vec<String>) -> Self {
        Self {
            jpreprocess,
            parsed,
        }
    }

    fn fix_phone_tone(&self, phone_tone_list: Vec<(String, i32)>) -> Result<Vec<(String, i32)>> {
        let tone_values: HashSet<i32> = phone_tone_list
            .iter()
            .map(|(_letter, tone)| *tone)
            .collect();
        if tone_values.len() == 1 {
            assert!(tone_values == hash_set![0], "{tone_values:?}");
            Ok(phone_tone_list)
        } else if tone_values.len() == 2 {
            if tone_values == hash_set![0, 1] {
                Ok(phone_tone_list)
            } else if tone_values == hash_set![-1, 0] {
                Ok(phone_tone_list
                    .iter()
                    .map(|x| {
                        let new_tone = if x.1 == -1 { 0 } else { 1 };
                        (x.0.clone(), new_tone)
                    })
                    .collect())
            } else {
                Err(Error::ValueError("Invalid tone values 0".to_string()))
            }
        } else {
            Err(Error::ValueError("Invalid tone values 1".to_string()))
        }
    }

    pub fn g2p(&self) -> Result<(Vec<String>, Vec<i32>, Vec<i32>)> {
        let phone_tone_list_wo_punct = self.g2phone_tone_wo_punct()?;
        let (seq_text, seq_kata) = self.text_to_seq_kata()?;
        let sep_phonemes = JTalkProcess::handle_long(
            seq_kata
                .iter()
                .map(|x| JTalkProcess::kata_to_phoneme_list(x.clone()).unwrap())
                .collect(),
        );
        let phone_w_punct: Vec<String> = sep_phonemes
            .iter()
            .flat_map(|x| x.iter())
            .cloned()
            .collect();

        let mut phone_tone_list =
            JTalkProcess::align_tones(phone_w_punct, phone_tone_list_wo_punct)?;

        let mut sep_tokenized: Vec<Vec<String>> = Vec::new();
        for seq_text_item in &seq_text {
            let text = seq_text_item.clone();
            if !PUNCTUATIONS.contains(&text.as_str()) {
                sep_tokenized.push(text.chars().map(|x| x.to_string()).collect());
            } else {
                sep_tokenized.push(vec![text]);
            }
        }

        let mut word2ph = Vec::new();
        for (token, phoneme) in sep_tokenized.iter().zip(sep_phonemes.iter()) {
            let phone_len = phoneme.len() as i32;
            let word_len = token.len() as i32;
            word2ph.append(&mut JTalkProcess::distribute_phone(phone_len, word_len));
        }

        let mut new_phone_tone_list = vec![("_".to_string(), 0)];
        new_phone_tone_list.append(&mut phone_tone_list);
        new_phone_tone_list.push(("_".to_string(), 0));

        let mut new_word2ph = vec![1];
        new_word2ph.extend(word2ph.clone());
        new_word2ph.push(1);

        let phones: Vec<String> = new_phone_tone_list.iter().map(|(x, _)| x.clone()).collect();
        let tones: Vec<i32> = new_phone_tone_list.iter().map(|(_, x)| *x).collect();

        Ok((phones, tones, new_word2ph))
    }

    pub fn g2kana_tone(&self) -> Result<Vec<(String, i32)>> {
        let (phones, tones, _) = self.g2p()?;
        Ok(phone_tone_to_kana(phones, tones))
    }

    fn distribute_phone(n_phone: i32, n_word: i32) -> Vec<i32> {
        let mut phones_per_word = vec![0; n_word as usize];
        for _ in 0..n_phone {
            let min_task = phones_per_word.iter().min().unwrap();
            let min_index = phones_per_word
                .iter()
                .position(|&x| x == *min_task)
                .unwrap();
            phones_per_word[min_index] += 1;
        }
        phones_per_word
    }

    fn align_tones(
        phone_with_punct: Vec<String>,
        phone_tone_list: Vec<(String, i32)>,
    ) -> Result<Vec<(String, i32)>> {
        let mut result: Vec<(String, i32)> = Vec::new();
        let mut tone_index = 0;
        for phone in phone_with_punct.clone() {
            if tone_index >= phone_tone_list.len() {
                result.push((phone, 0));
            } else if phone == phone_tone_list[tone_index].0 {
                result.push((phone, phone_tone_list[tone_index].1));
                tone_index += 1;
            } else if PUNCTUATIONS.contains(&phone.as_str()) {
                result.push((phone, 0));
            } else {
                println!("phones {phone_with_punct:?}");
                println!("phone_tone_list: {phone_tone_list:?}");
                println!("result: {result:?}");
                println!("tone_index: {tone_index:?}");
                println!("phone: {phone:?}");
                return Err(Error::ValueError(format!("Mismatched phoneme: {phone}")));
            }
        }

        Ok(result)
    }

    fn handle_long(mut sep_phonemes: Vec<Vec<String>>) -> Vec<Vec<String>> {
        for i in 0..sep_phonemes.len() {
            if sep_phonemes[i].is_empty() {
                continue;
            }
            if sep_phonemes[i][0] == "ー" {
                if i != 0 {
                    let prev_phoneme = sep_phonemes[i - 1].last().unwrap();
                    if VOWELS.contains(&prev_phoneme.as_str()) {
                        sep_phonemes[i][0] = prev_phoneme.clone();
                    } else {
                        sep_phonemes[i][0] = "ー".to_string();
                    }
                } else {
                    sep_phonemes[i][0] = "ー".to_string();
                }
            }
            if sep_phonemes[i].contains(&"ー".to_string()) {
                for e in 0..sep_phonemes[i].len() {
                    if sep_phonemes[i][e] == "ー" {
                        sep_phonemes[i][e] =
                            sep_phonemes[i][e - 1].chars().last().unwrap().to_string();
                    }
                }
            }
        }
        sep_phonemes
    }

    fn kata_to_phoneme_list(mut text: String) -> Result<Vec<String>> {
        let chars: HashSet<String> = text.chars().map(|x| x.to_string()).collect();
        if chars.is_subset(&HashSet::from_iter(
            PUNCTUATIONS.iter().map(|x| x.to_string()),
        )) {
            return Ok(text.chars().map(|x| x.to_string()).collect());
        }
        if !KATAKANA_PATTERN.is_match(&text) {
            return Err(Error::ValueError(format!(
                "Input must be katakana only: {text}"
            )));
        }

        for mora in MORA_PATTERN.iter() {
            let mora = mora.to_string();
            let (consonant, vowel) = MORA_KATA_TO_MORA_PHONEMES.get(&mora).unwrap();
            if consonant.is_none() {
                text = text.replace(&mora, &format!(" {vowel}"));
            } else {
                text = text.replace(
                    &mora,
                    &format!(" {} {}", consonant.as_ref().unwrap(), vowel),
                );
            }
        }

        let long_replacement = |m: &regex::Captures| {
            let result = m.get(1).unwrap().as_str().to_string();
            let mut second = String::new();
            for _ in 0..m.get(2).unwrap().as_str().char_indices().count() {
                second += &format!(" {}", m.get(1).unwrap().as_str());
            }
            result + &second
        };
        text = LONG_PATTERN
            .replace_all(&text, long_replacement)
            .to_string();

        let data = text.trim().split(' ').map(|x| x.to_string()).collect();

        Ok(data)
    }

    pub fn text_to_seq_kata(&self) -> Result<(Vec<String>, Vec<String>)> {
        let mut seq_kata = vec![];
        let mut seq_text = vec![];

        for parts in &self.parsed {
            let (string, pron) = self.parse_to_string_and_pron(parts.clone());
            let mut yomi = pron.replace('’', "");
            let word = replace_punctuation(string);
            assert!(!yomi.is_empty(), "Empty yomi: {word}");
            if yomi == "、" {
                if !word
                    .chars()
                    .all(|x| PUNCTUATIONS.contains(&x.to_string().as_str()))
                {
                    yomi = "'".repeat(word.len());
                } else {
                    yomi = word.clone();
                }
            } else if yomi == "？" {
                assert!(word == "?", "yomi `？` comes from: {word}");
                yomi = "?".to_string();
            }
            seq_text.push(word);
            seq_kata.push(yomi);
        }
        Ok((seq_text, seq_kata))
    }

    fn parse_to_string_and_pron(&self, parts: String) -> (String, String) {
        let part_lists: Vec<String> = parts.split(',').map(|x| x.to_string()).collect();
        (part_lists[0].clone(), part_lists[9].clone())
    }

    fn g2phone_tone_wo_punct(&self) -> Result<Vec<(String, i32)>> {
        let prosodies = self.g2p_prosody()?;

        let mut results: Vec<(String, i32)> = Vec::new();
        let mut current_phrase: Vec<(String, i32)> = Vec::new();
        let mut current_tone = 0;

        for (i, letter) in prosodies.iter().enumerate() {
            if letter == "^" {
                assert!(i == 0);
            } else if ["$", "?", "_", "#"].contains(&letter.as_str()) {
                results.extend(self.fix_phone_tone(current_phrase.clone())?);
                if ["$", "?"].contains(&letter.as_str()) {
                    assert!(i == prosodies.len() - 1);
                }
                current_phrase = Vec::new();
                current_tone = 0;
            } else if letter == "[" {
                current_tone += 1;
            } else if letter == "]" {
                current_tone -= 1;
            } else {
                let new_letter = if letter == "cl" {
                    "q".to_string()
                } else {
                    letter.clone()
                };
                current_phrase.push((new_letter, current_tone));
            }
        }

        Ok(results)
    }

    fn g2p_prosody(&self) -> Result<Vec<String>> {
        let labels = self.jpreprocess.make_label(self.parsed.clone());

        let mut phones: Vec<String> = Vec::new();
        for (i, label) in labels.iter().enumerate() {
            let mut p3 = label.phoneme.c.clone().unwrap();
            if "AIUEO".contains(&p3) {
                // 文字をlowerする
                p3 = p3.to_lowercase();
            }
            if p3 == "sil" {
                assert!(i == 0 || i == labels.len() - 1);
                if i == 0 {
                    phones.push("^".to_string());
                } else if i == labels.len() - 1 {
                    let e3 = label.accent_phrase_prev.clone().unwrap().is_interrogative;
                    if e3 {
                        phones.push("$".to_string());
                    } else {
                        phones.push("?".to_string());
                    }
                }
                continue;
            } else if p3 == "pau" {
                phones.push("_".to_string());
                continue;
            } else {
                phones.push(p3.clone());
            }

            let a1 = if let Some(mora) = &label.mora {
                mora.relative_accent_position as i32
            } else {
                -50
            };
            let a2 = if let Some(mora) = &label.mora {
                mora.position_forward as i32
            } else {
                -50
            };
            let a3 = if let Some(mora) = &label.mora {
                mora.position_backward as i32
            } else {
                -50
            };

            let f1 = if let Some(accent_phrase) = &label.accent_phrase_curr {
                accent_phrase.mora_count as i32
            } else {
                -50
            };

            let a2_next = if let Some(mora) = &labels[i + 1].mora {
                mora.position_forward as i32
            } else {
                -50
            };

            if a3 == 1 && a2_next == 1 && "aeiouAEIOUNcl".contains(&p3) {
                phones.push("#".to_string());
            } else if a1 == 0 && a2_next == a2 + 1 && a2 != f1 {
                phones.push("]".to_string());
            } else if a2 == 1 && a2_next == 2 {
                phones.push("[".to_string());
            }
        }

        Ok(phones)
    }
}
