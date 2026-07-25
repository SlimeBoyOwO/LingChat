use crate::error::{Error, Result};
use crate::{jtalk, model, style, tokenizer, tts_util};
#[cfg(feature = "aivmx")]
use base64::prelude::{Engine as _, BASE64_STANDARD};
#[cfg(feature = "aivmx")]
use ndarray::ShapeBuilder;
use ndarray::{concatenate, Array1, Array2, Array3, Axis};
use ort::session::Session;
#[cfg(feature = "aivmx")]
use std::io::Cursor;
use tokenizers::Tokenizer;

#[derive(PartialEq, Eq, Clone)]
pub struct TTSIdent(String);

impl std::fmt::Display for TTSIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)?;
        Ok(())
    }
}

impl<S> From<S> for TTSIdent
where
    S: AsRef<str>,
{
    fn from(value: S) -> Self {
        TTSIdent(value.as_ref().to_string())
    }
}

pub struct TTSModel {
    vits2: Option<Session>,
    style_vectors: Array2<f32>,
    ident: TTSIdent,
    bytes: Option<Vec<u8>>,
}

/// High-level Style-Bert-VITS2's API
pub struct TTSModelHolder {
    tokenizer: Tokenizer,
    bert: Session,
    models: Vec<TTSModel>,
    pub jtalk: jtalk::JTalk,
    max_loaded_models: Option<usize>,
}

impl TTSModelHolder {
    /// Initialize a new TTSModelHolder
    ///
    /// # Examples
    ///
    /// ```rs
    /// let mut tts_holder = TTSModelHolder::new(std::fs::read("deberta.onnx")?, std::fs::read("tokenizer.json")?, None)?;
    /// ```
    pub fn new<P: AsRef<[u8]>>(
        bert_model_bytes: P,
        tokenizer_bytes: P,
        max_loaded_models: Option<usize>,
    ) -> Result<Self> {
        let bert = model::load_model(bert_model_bytes, true)?;
        let jtalk = jtalk::JTalk::new()?;
        let tokenizer = tokenizer::get_tokenizer(tokenizer_bytes)?;
        Ok(TTSModelHolder {
            bert,
            models: vec![],
            jtalk,
            tokenizer,
            max_loaded_models,
        })
    }

    /// Return a list of model names
    pub fn models(&self) -> Vec<String> {
        self.models.iter().map(|m| m.ident.to_string()).collect()
    }

    #[cfg(feature = "aivmx")]
    pub fn load_aivmx<I: Into<TTSIdent>, P: AsRef<[u8]>>(
        &mut self,
        ident: I,
        aivmx_bytes: P,
    ) -> Result<()> {
        let ident = ident.into();
        if self.find_model(ident.clone()).is_err() {
            let mut load = true;
            if let Some(max) = self.max_loaded_models {
                if self.models.iter().filter(|x| x.vits2.is_some()).count() >= max {
                    load = false;
                }
            }
            let model = model::load_model(&aivmx_bytes, false)?;
            let metadata = model.metadata()?;
            if let Some(aivm_style_vectors) = metadata.custom("aivm_style_vectors") {
                let aivm_style_vectors = BASE64_STANDARD.decode(aivm_style_vectors)?;
                let style_vectors = Cursor::new(&aivm_style_vectors);
                let reader = npyz::NpyFile::new(style_vectors)?;
                let style_vectors = {
                    let shape = reader.shape().to_vec();
                    let order = reader.order();
                    let data = reader.into_vec::<f32>()?;
                    let shape = match shape[..] {
                        [i1, i2] => [i1 as usize, i2 as usize],
                        _ => panic!("expected 2D array"),
                    };
                    let true_shape = shape.set_f(order == npyz::Order::Fortran);
                    ndarray::Array2::from_shape_vec(true_shape, data)?
                };
                drop(metadata);
                self.models.push(TTSModel {
                    vits2: if load { Some(model) } else { None },
                    bytes: if self.max_loaded_models.is_some() {
                        Some(aivmx_bytes.as_ref().to_vec())
                    } else {
                        None
                    },
                    ident,
                    style_vectors,
                })
            }
        }
        Ok(())
    }

    /// Load a .sbv2 file binary
    ///
    /// # Examples
    ///
    /// ```rs
    /// tts_holder.load_sbv2file("tsukuyomi", std::fs::read("tsukuyomi.sbv2")?)?;
    /// ```
    pub fn load_sbv2file<I: Into<TTSIdent>, P: AsRef<[u8]>>(
        &mut self,
        ident: I,
        sbv2_bytes: P,
    ) -> Result<()> {
        let (style_vectors, vits2) = crate::sbv2file::parse_sbv2file(sbv2_bytes)?;
        self.load(ident, style_vectors, vits2)?;
        Ok(())
    }

    /// Load a style vector and onnx model binary
    ///
    /// # Examples
    ///
    /// ```rs
    /// tts_holder.load("tsukuyomi", std::fs::read("style_vectors.json")?, std::fs::read("model.onnx")?)?;
    /// ```
    pub fn load<I: Into<TTSIdent>, P: AsRef<[u8]>>(
        &mut self,
        ident: I,
        style_vectors_bytes: P,
        vits2_bytes: P,
    ) -> Result<()> {
        let ident = ident.into();
        if self.find_model(ident.clone()).is_err() {
            let mut load = true;
            if let Some(max) = self.max_loaded_models {
                if self.models.iter().filter(|x| x.vits2.is_some()).count() >= max {
                    load = false;
                }
            }
            self.models.push(TTSModel {
                vits2: if load {
                    Some(model::load_model(&vits2_bytes, false)?)
                } else {
                    None
                },
                style_vectors: style::load_style(style_vectors_bytes)?,
                ident,
                bytes: if self.max_loaded_models.is_some() {
                    Some(vits2_bytes.as_ref().to_vec())
                } else {
                    None
                },
            })
        }
        Ok(())
    }

    /// Unload a model
    pub fn unload<I: Into<TTSIdent>>(&mut self, ident: I) -> bool {
        let ident = ident.into();
        if let Some((i, _)) = self
            .models
            .iter()
            .enumerate()
            .find(|(_, m)| m.ident == ident)
        {
            self.models.remove(i);
            true
        } else {
            false
        }
    }

    /// Parse text and return the input for synthesize
    ///
    /// # Note
    /// This function is for low-level usage, use `easy_synthesize` for high-level usage.
    #[allow(clippy::type_complexity)]
    pub fn parse_text(
        &mut self,
        text: &str,
    ) -> Result<(Array2<f32>, Array1<i64>, Array1<i64>, Array1<i64>)> {
        crate::tts_util::parse_text_blocking(
            text,
            None,
            &self.jtalk,
            &self.tokenizer,
            |token_ids, attention_masks| {
                crate::bert::predict(&mut self.bert, token_ids, attention_masks)
            },
        )
    }

    #[allow(clippy::type_complexity)]
    pub fn parse_text_neo(
        &mut self,
        text: String,
        given_tones: Option<Vec<i32>>,
    ) -> Result<(Array2<f32>, Array1<i64>, Array1<i64>, Array1<i64>)> {
        crate::tts_util::parse_text_blocking(
            &text,
            given_tones,
            &self.jtalk,
            &self.tokenizer,
            |token_ids, attention_masks| {
                crate::bert::predict(&mut self.bert, token_ids, attention_masks)
            },
        )
    }

    fn find_model<I: Into<TTSIdent>>(&mut self, ident: I) -> Result<&mut TTSModel> {
        let ident = ident.into();
        self.models
            .iter_mut()
            .find(|m| m.ident == ident)
            .ok_or(Error::ModelNotFoundError(ident.to_string()))
    }
    fn find_and_load_model<I: Into<TTSIdent>>(&mut self, ident: I) -> Result<bool> {
        let ident = ident.into();
        // Locate target model entry
        let target_index = self
            .models
            .iter()
            .position(|m| m.ident == ident)
            .ok_or(Error::ModelNotFoundError(ident.to_string()))?;

        // Already loaded
        if self.models[target_index].vits2.is_some() {
            return Ok(true);
        }

        // Get bytes to build a Session
        let bytes = self.models[target_index]
            .bytes
            .clone()
            .ok_or(Error::ModelNotFoundError(ident.to_string()))?;

        // Enforce max loaded models by evicting a different loaded model's session, not removing the entry
        if let Some(max) = self.max_loaded_models {
            let loaded_count = self.models.iter().filter(|m| m.vits2.is_some()).count();
            if loaded_count >= max {
                if let Some(evict_index) = self
                    .models
                    .iter()
                    .position(|m| m.vits2.is_some() && m.ident != ident)
                {
                    // Drop only the session to free memory; keep bytes/style for future reload
                    self.models[evict_index].vits2 = None;
                }
            }
        }

        // Build and set session in-place for the target model
        let s = model::load_model(&bytes, false)?;
        self.models[target_index].vits2 = Some(s);
        Ok(true)
    }

    /// Get style vector by style id and weight
    ///
    /// # Note
    /// This function is for low-level usage, use `easy_synthesize` for high-level usage.
    pub fn get_style_vector<I: Into<TTSIdent>>(
        &mut self,
        ident: I,
        style_id: i32,
        weight: f32,
    ) -> Result<Array1<f32>> {
        style::get_style_vector(&self.find_model(ident)?.style_vectors, style_id, weight)
    }

    /// Synthesize text to audio
    ///
    /// # Examples
    ///
    /// ```rs
    /// let audio = tts_holder.easy_synthesize("tsukuyomi", "こんにちは", 0, SynthesizeOptions::default())?;
    /// ```
    pub fn easy_synthesize<I: Into<TTSIdent> + Copy>(
        &mut self,
        ident: I,
        text: &str,
        style_id: i32,
        speaker_id: i64,
        options: SynthesizeOptions,
    ) -> Result<Vec<u8>> {
        self.find_and_load_model(ident)?;
        let style_vector = self.get_style_vector(ident, style_id, options.style_weight)?;
        let audio_array = if options.split_sentences {
            let texts: Vec<&str> = text.split('\n').collect();
            let mut audios = vec![];
            for (i, t) in texts.iter().enumerate() {
                if t.is_empty() {
                    continue;
                }
                let (bert_ori, phones, tones, lang_ids) = self.parse_text(t)?;

                let vits2 = self
                    .find_model(ident)?
                    .vits2
                    .as_mut()
                    .ok_or(Error::ModelNotFoundError(ident.into().to_string()))?;
                let audio = model::synthesize(
                    vits2,
                    bert_ori.to_owned(),
                    phones,
                    Array1::from_vec(vec![speaker_id]),
                    tones,
                    lang_ids,
                    style_vector.clone(),
                    options.sdp_ratio,
                    options.length_scale,
                    0.677,
                    0.8,
                )?;
                audios.push(audio.clone());
                if i != texts.len() - 1 {
                    audios.push(Array3::zeros((1, 1, 22050)));
                }
            }
            concatenate(
                Axis(2),
                &audios.iter().map(|x| x.view()).collect::<Vec<_>>(),
            )?
        } else {
            let (bert_ori, phones, tones, lang_ids) = self.parse_text(text)?;

            let vits2 = self
                .find_model(ident)?
                .vits2
                .as_mut()
                .ok_or(Error::ModelNotFoundError(ident.into().to_string()))?;
            model::synthesize(
                vits2,
                bert_ori.to_owned(),
                phones,
                Array1::from_vec(vec![speaker_id]),
                tones,
                lang_ids,
                style_vector,
                options.sdp_ratio,
                options.length_scale,
                0.677,
                0.8,
            )?
        };
        tts_util::array_to_vec(audio_array)
    }

    pub fn easy_synthesize_neo<I: Into<TTSIdent> + Copy>(
        &mut self,
        ident: I,
        text: &str,
        given_tones: Option<Vec<i32>>,
        style_id: i32,
        speaker_id: i64,
        options: SynthesizeOptions,
    ) -> Result<Vec<u8>> {
        self.find_and_load_model(ident)?;
        let style_vector = self.get_style_vector(ident, style_id, options.style_weight)?;
        let audio_array = if options.split_sentences {
            let texts: Vec<&str> = text.split('\n').collect();
            let mut audios = vec![];
            for (i, t) in texts.iter().enumerate() {
                if t.is_empty() {
                    continue;
                }
                let (bert_ori, phones, tones, lang_ids) =
                    self.parse_text_neo(t.to_string(), given_tones.clone())?;

                let vits2 = self
                    .find_model(ident)?
                    .vits2
                    .as_mut()
                    .ok_or(Error::ModelNotFoundError(ident.into().to_string()))?;
                let audio = model::synthesize(
                    vits2,
                    bert_ori.to_owned(),
                    phones,
                    Array1::from_vec(vec![speaker_id]),
                    tones,
                    lang_ids,
                    style_vector.clone(),
                    options.sdp_ratio,
                    options.length_scale,
                    0.677,
                    0.8,
                )?;
                audios.push(audio.clone());
                if i != texts.len() - 1 {
                    audios.push(Array3::zeros((1, 1, 22050)));
                }
            }
            concatenate(
                Axis(2),
                &audios.iter().map(|x| x.view()).collect::<Vec<_>>(),
            )?
        } else {
            let (bert_ori, phones, tones, lang_ids) = self.parse_text(text)?;

            let vits2 = self
                .find_model(ident)?
                .vits2
                .as_mut()
                .ok_or(Error::ModelNotFoundError(ident.into().to_string()))?;
            model::synthesize(
                vits2,
                bert_ori.to_owned(),
                phones,
                Array1::from_vec(vec![speaker_id]),
                tones,
                lang_ids,
                style_vector,
                options.sdp_ratio,
                options.length_scale,
                0.677,
                0.8,
            )?
        };
        tts_util::array_to_vec(audio_array)
    }
}

/// Synthesize options
///
/// # Fields
/// - `sdp_ratio`: SDP ratio
/// - `length_scale`: Length scale
/// - `style_weight`: Style weight
/// - `split_sentences`: Split sentences
pub struct SynthesizeOptions {
    pub sdp_ratio: f32,
    pub length_scale: f32,
    pub style_weight: f32,
    pub split_sentences: bool,
}

impl Default for SynthesizeOptions {
    fn default() -> Self {
        SynthesizeOptions {
            sdp_ratio: 0.0,
            length_scale: 1.0,
            style_weight: 1.0,
            split_sentences: true,
        }
    }
}
