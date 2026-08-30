// Curated asset catalog for local TTS models.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub id: String,
    pub kind: AssetKind,
    pub display_name: String,
    pub language: String,
    pub size_bytes: u64,
    pub download_url: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_id: Option<String>,
    /// 下载此资产时一并下载的其他资产 ID（如 deberta → tokenizer，ling-v2 → style_vectors）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundled_assets: Vec<String>,
    /// Sherpa-ONNX 模型元数据（仅 SherpaOnnx 类型使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sherpa_meta: Option<SherpaOnnxMeta>,
    /// HF Mirror 多文件源（相对路径 → 下载 URL）。优先于 download_url 使用。
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub hf_files: HashMap<String, String>,
    /// ModelScope 目录源（`<repo>/<子目录>`，如 `gomodels/sherpa/matcha-icefall-zh-baker`）。
    /// 设置后优先通过 ModelScope repo API 枚举目录内全部 blob 逐一下载到目标目录，
    /// API 不可用时回退到 [`static_subdir_files`] 内置清单（层目录如 espeak-ng-data、dict 都能还原）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modelscope_dir: Option<String>,
    /// GitHub Releases tar.bz2 回退下载地址（当首选源下载失败时使用）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_fallback_url: Option<String>,
}

/// Sherpa-ONNX 模型元数据，描述解压后模型目录的属性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SherpaOnnxMeta {
    /// 模型类型（vits / fastspeech2）
    pub model_type: String,
    /// 模型语言（zh / en / ja / ko）
    pub language: String,
    /// 音色名称
    pub voice: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Bert,
    Voice,
    StyleVectors,
    SherpaOnnx,
}

pub fn catalog() -> Vec<AssetEntry> {
    vec![
        AssetEntry {
            id: "deberta".into(),
            kind: AssetKind::Bert,
            display_name: "DeBERTa-v3-base (Japanese BERT)".into(),
            language: "ja".into(),
            size_bytes: 278_000_000,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/DeBERTa.onnx/resolve/master/deberta.onnx".into(),
            source: "lingchat-research-studio/DeBERTa.onnx".into(),
            voice_id: None,
            bundled_assets: vec!["deberta-tokenizer".into()],
            sherpa_meta: None,
            hf_files: HashMap::new(),
            modelscope_dir: None,
            github_fallback_url: None,
        },
        AssetEntry {
            id: "deberta-tokenizer".into(),
            kind: AssetKind::Bert,
            display_name: "DeBERTa-v3-base Tokenizer".into(),
            language: "ja".into(),
            size_bytes: 2_100_000,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/DeBERTa.onnx/resolve/master/tokenizer.json".into(),
            source: "lingchat-research-studio/DeBERTa.onnx".into(),
            voice_id: None,
            bundled_assets: vec![],
            sherpa_meta: None,
            hf_files: HashMap::new(),
            modelscope_dir: None,
            github_fallback_url: None,
        },
        AssetEntry {
            id: "ling-v2".into(),
            kind: AssetKind::Voice,
            display_name: "Ling-v2 (Japanese)".into(),
            language: "ja".into(),
            size_bytes: 249_000_000,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/sbv2api-model-Ling-v2-onnx/resolve/master/sbv2api-model-Ling-v2-onnx.onnx".into(),
            source: "lingchat-research-studio/sbv2api-model-Ling-v2-onnx".into(),
            voice_id: None,
            bundled_assets: vec!["ling-v2-style".into()],
            sherpa_meta: None,
            hf_files: HashMap::new(),
            modelscope_dir: None,
            github_fallback_url: None,
        },
        AssetEntry {
            id: "ling-v2-style".into(),
            kind: AssetKind::StyleVectors,
            display_name: "Ling-v2 Style Vectors".into(),
            language: "ja".into(),
            size_bytes: 7_400,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/sbv2api-model-Ling-v2-onnx/resolve/master/style_vectors.json".into(),
            source: "lingchat-research-studio/sbv2api-model-Ling-v2-onnx".into(),
            voice_id: Some("ling-v2".into()),
            bundled_assets: vec![],
            sherpa_meta: None,
            hf_files: HashMap::new(),
            modelscope_dir: None,
            github_fallback_url: None,
        },
        // ── Sherpa-ONNX TTS 模型 ──
        // 全部来自 ModelScope 镜像仓库 gomodels/sherpa，按子目录整目录下载
        // （运行时通过 ModelScope repo API 枚举目录内全部文件逐一拉取，可还原
        //  espeak-ng-data、dict 等嵌套目录），GitHub Releases tar.bz2 作为整包回退。
        AssetEntry {
            id: "sherpa-matcha-zh".into(),
            kind: AssetKind::SherpaOnnx,
            display_name: "Sherpa Matcha TTS 中文 (Baker 女声)".into(),
            language: "zh".into(),
            size_bytes: 139_000_000,
            download_url: String::new(),
            source: "gomodels/sherpa/matcha-icefall-zh-baker".into(),
            voice_id: None,
            bundled_assets: vec![],
            sherpa_meta: Some(SherpaOnnxMeta { model_type: "matcha".into(), language: "zh".into(), voice: "female".into() }),
            hf_files: HashMap::new(),
            modelscope_dir: Some("gomodels/sherpa/matcha-icefall-zh-baker".into()),
            github_fallback_url: Some("https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/matcha-icefall-zh-baker.tar.bz2".into()),
        },
        AssetEntry {
            id: "sherpa-matcha-zh-en".into(),
            kind: AssetKind::SherpaOnnx,
            display_name: "Sherpa Matcha TTS 中英双语 (浊辅音女声)".into(),
            language: "zh".into(),
            size_bytes: 285_000_000,
            download_url: String::new(),
            source: "gomodels/sherpa/matcha_tts_zh_en_20251010".into(),
            voice_id: None,
            bundled_assets: vec![],
            sherpa_meta: Some(SherpaOnnxMeta { model_type: "matcha".into(), language: "zh".into(), voice: "female".into() }),
            hf_files: HashMap::new(),
            modelscope_dir: Some("gomodels/sherpa/matcha_tts_zh_en_20251010".into()),
            github_fallback_url: Some("https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/matcha-icefall-zh-en.tar.bz2".into()),
        },
        AssetEntry {
            id: "sherpa-kokoro-multi-lang".into(),
            kind: AssetKind::SherpaOnnx,
            display_name: "Sherpa Kokoro 多语言 (INT8, 200+ 音色)".into(),
            language: "zh".into(),
            size_bytes: 205_000_000,
            download_url: String::new(),
            source: "gomodels/sherpa/kokoro-int8-multi-lang-v1_1".into(),
            voice_id: None,
            bundled_assets: vec![],
            sherpa_meta: Some(SherpaOnnxMeta { model_type: "kokoro".into(), language: "zh".into(), voice: "multi".into() }),
            hf_files: HashMap::new(),
            modelscope_dir: Some("gomodels/sherpa/kokoro-int8-multi-lang-v1_1".into()),
            github_fallback_url: Some("https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-int8-multi-lang-v1_1.tar.bz2".into()),
        },
        AssetEntry {
            id: "sherpa-zipvoice-zh-en".into(),
            kind: AssetKind::SherpaOnnx,
            display_name: "Sherpa ZipVoice 中英双语 (蒸馏, 24kHz)".into(),
            language: "zh".into(),
            size_bytes: 692_000_000,
            download_url: String::new(),
            source: "gomodels/sherpa/sherpa-onnx-zipvoice-distill-zh-en-emilia".into(),
            voice_id: None,
            bundled_assets: vec![],
            sherpa_meta: Some(SherpaOnnxMeta { model_type: "zipvoice".into(), language: "zh".into(), voice: "multi".into() }),
            hf_files: HashMap::new(),
            modelscope_dir: Some("gomodels/sherpa/sherpa-onnx-zipvoice-distill-zh-en-emilia".into()),
            github_fallback_url: Some("https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-zipvoice-distill-zh-en-emilia.tar.bz2".into()),
        },
    ]
}

pub fn find(id: &str) -> Option<AssetEntry> {
    catalog().into_iter().find(|a| a.id == id)
}

pub fn all_assets() -> Vec<AssetEntry> {
    catalog()
}

// 内置 ModelScope 目录静态文件清单，见 `static_subdir_files`。
// 生成自 gomodels/sherpa 仓库（ms2.json），路径相对各自的子目录。

/// 三个大模型共享的 `espeak-ng-data` 文件集（355 个，已比对三仓库一致）。
pub const ESPEAK_NG_DATA_FILES: &[&str] = &[
    "espeak-ng-data/voices/!v/adam", "espeak-ng-data/lang/gmw/af", "espeak-ng-data/af_dict", "espeak-ng-data/voices/!v/Alex", "espeak-ng-data/voices/!v/Alicia", "espeak-ng-data/lang/sem/am", "espeak-ng-data/am_dict", "espeak-ng-data/lang/roa/an",
    "espeak-ng-data/an_dict", "espeak-ng-data/voices/!v/Andrea", "espeak-ng-data/voices/!v/Andy", "espeak-ng-data/voices/!v/anika", "espeak-ng-data/voices/!v/anikaRobot", "espeak-ng-data/voices/!v/Annie", "espeak-ng-data/voices/!v/announcer", "espeak-ng-data/voices/!v/antonio",
    "espeak-ng-data/voices/!v/AnxiousAndy", "espeak-ng-data/lang/sem/ar", "espeak-ng-data/ar_dict", "espeak-ng-data/lang/inc/as", "espeak-ng-data/as_dict", "espeak-ng-data/voices/!v/aunty", "espeak-ng-data/lang/trk/az", "espeak-ng-data/az_dict",
    "espeak-ng-data/lang/trk/ba", "espeak-ng-data/ba_dict", "espeak-ng-data/lang/zle/be", "espeak-ng-data/be_dict", "espeak-ng-data/voices/!v/belinda", "espeak-ng-data/voices/!v/benjamin", "espeak-ng-data/lang/zls/bg", "espeak-ng-data/bg_dict",
    "espeak-ng-data/lang/inc/bn", "espeak-ng-data/bn_dict", "espeak-ng-data/voices/!v/boris", "espeak-ng-data/lang/inc/bpy", "espeak-ng-data/bpy_dict", "espeak-ng-data/lang/zls/bs", "espeak-ng-data/bs_dict", "espeak-ng-data/lang/roa/ca",
    "espeak-ng-data/ca_dict", "espeak-ng-data/voices/!v/caleb", "espeak-ng-data/lang/iro/chr", "espeak-ng-data/chr_dict", "espeak-ng-data/lang/sit/cmn", "espeak-ng-data/lang/sit/cmn-Latn-pinyin", "espeak-ng-data/cmn_dict", "espeak-ng-data/voices/!v/croak",
    "espeak-ng-data/lang/zlw/cs", "espeak-ng-data/cs_dict", "espeak-ng-data/lang/trk/cv", "espeak-ng-data/cv_dict", "espeak-ng-data/lang/cel/cy", "espeak-ng-data/cy_dict", "espeak-ng-data/lang/gmq/da", "espeak-ng-data/da_dict",
    "espeak-ng-data/voices/!v/david", "espeak-ng-data/lang/gmw/de", "espeak-ng-data/de_dict", "espeak-ng-data/voices/!v/Demonic", "espeak-ng-data/voices/!v/Denis", "espeak-ng-data/voices/!v/Diogo", "espeak-ng-data/voices/!v/ed", "espeak-ng-data/voices/!v/edward",
    "espeak-ng-data/voices/!v/edward2", "espeak-ng-data/lang/grk/el", "espeak-ng-data/el_dict", "espeak-ng-data/lang/gmw/en", "espeak-ng-data/lang/gmw/en-029", "espeak-ng-data/lang/gmw/en-GB-scotland", "espeak-ng-data/lang/gmw/en-GB-x-gbclan", "espeak-ng-data/lang/gmw/en-GB-x-gbcwmd",
    "espeak-ng-data/lang/gmw/en-GB-x-rp", "espeak-ng-data/lang/gmw/en-US", "espeak-ng-data/lang/gmw/en-US-nyc", "espeak-ng-data/en_dict", "espeak-ng-data/lang/art/eo", "espeak-ng-data/eo_dict", "espeak-ng-data/lang/roa/es", "espeak-ng-data/lang/roa/es-419",
    "espeak-ng-data/es_dict", "espeak-ng-data/lang/urj/et", "espeak-ng-data/et_dict", "espeak-ng-data/lang/eu", "espeak-ng-data/eu_dict", "espeak-ng-data/voices/!v/f1", "espeak-ng-data/voices/!v/f2", "espeak-ng-data/voices/!v/f3",
    "espeak-ng-data/voices/!v/f4", "espeak-ng-data/voices/!v/f5", "espeak-ng-data/lang/ira/fa", "espeak-ng-data/lang/ira/fa-Latn", "espeak-ng-data/fa_dict", "espeak-ng-data/voices/!v/fast", "espeak-ng-data/lang/urj/fi", "espeak-ng-data/fi_dict",
    "espeak-ng-data/lang/roa/fr", "espeak-ng-data/lang/roa/fr-BE", "espeak-ng-data/lang/roa/fr-CH", "espeak-ng-data/fr_dict", "espeak-ng-data/lang/cel/ga", "espeak-ng-data/ga_dict", "espeak-ng-data/lang/cel/gd", "espeak-ng-data/gd_dict",
    "espeak-ng-data/voices/!v/Gene", "espeak-ng-data/voices/!v/Gene2", "espeak-ng-data/lang/sai/gn", "espeak-ng-data/gn_dict", "espeak-ng-data/voices/!v/grandma", "espeak-ng-data/voices/!v/grandpa", "espeak-ng-data/lang/grk/grc", "espeak-ng-data/grc_dict",
    "espeak-ng-data/lang/inc/gu", "espeak-ng-data/gu_dict", "espeak-ng-data/voices/!v/gustave", "espeak-ng-data/lang/sit/hak", "espeak-ng-data/hak_dict", "espeak-ng-data/lang/map/haw", "espeak-ng-data/haw_dict", "espeak-ng-data/lang/sem/he",
    "espeak-ng-data/he_dict", "espeak-ng-data/voices/!v/Henrique", "espeak-ng-data/lang/inc/hi", "espeak-ng-data/hi_dict", "espeak-ng-data/lang/zls/hr", "espeak-ng-data/hr_dict", "espeak-ng-data/lang/roa/ht", "espeak-ng-data/ht_dict",
    "espeak-ng-data/lang/urj/hu", "espeak-ng-data/hu_dict", "espeak-ng-data/voices/!v/Hugo", "espeak-ng-data/lang/ine/hy", "espeak-ng-data/hy_dict", "espeak-ng-data/lang/ine/hyw", "espeak-ng-data/lang/art/ia", "espeak-ng-data/ia_dict",
    "espeak-ng-data/voices/!v/ian", "espeak-ng-data/lang/poz/id", "espeak-ng-data/id_dict", "espeak-ng-data/intonations", "espeak-ng-data/lang/art/io", "espeak-ng-data/io_dict", "espeak-ng-data/lang/gmq/is", "espeak-ng-data/is_dict",
    "espeak-ng-data/lang/roa/it", "espeak-ng-data/it_dict", "espeak-ng-data/voices/!v/iven", "espeak-ng-data/voices/!v/iven2", "espeak-ng-data/voices/!v/iven3", "espeak-ng-data/voices/!v/iven4", "espeak-ng-data/lang/jpx/ja", "espeak-ng-data/ja_dict",
    "espeak-ng-data/voices/!v/Jacky", "espeak-ng-data/lang/art/jbo", "espeak-ng-data/jbo_dict", "espeak-ng-data/voices/!v/john", "espeak-ng-data/lang/ccs/ka", "espeak-ng-data/ka_dict", "espeak-ng-data/voices/!v/kaukovalta", "espeak-ng-data/lang/trk/kk",
    "espeak-ng-data/kk_dict", "espeak-ng-data/lang/esx/kl", "espeak-ng-data/kl_dict", "espeak-ng-data/voices/!v/klatt", "espeak-ng-data/voices/!v/klatt2", "espeak-ng-data/voices/!v/klatt3", "espeak-ng-data/voices/!v/klatt4", "espeak-ng-data/voices/!v/klatt5",
    "espeak-ng-data/voices/!v/klatt6", "espeak-ng-data/lang/dra/kn", "espeak-ng-data/kn_dict", "espeak-ng-data/lang/ko", "espeak-ng-data/ko_dict", "espeak-ng-data/lang/inc/kok", "espeak-ng-data/kok_dict", "espeak-ng-data/lang/ira/ku",
    "espeak-ng-data/ku_dict", "espeak-ng-data/lang/trk/ky", "espeak-ng-data/ky_dict", "espeak-ng-data/lang/itc/la", "espeak-ng-data/la_dict", "espeak-ng-data/lang/gmw/lb", "espeak-ng-data/lb_dict", "espeak-ng-data/voices/!v/Lee",
    "espeak-ng-data/lang/art/lfn", "espeak-ng-data/lfn_dict", "espeak-ng-data/voices/!v/linda", "espeak-ng-data/lang/bat/lt", "espeak-ng-data/lt_dict", "espeak-ng-data/lang/bat/ltg", "espeak-ng-data/lang/bat/lv", "espeak-ng-data/lv_dict",
    "espeak-ng-data/voices/!v/m1", "espeak-ng-data/voices/!v/m2", "espeak-ng-data/voices/!v/m3", "espeak-ng-data/voices/!v/m4", "espeak-ng-data/voices/!v/m5", "espeak-ng-data/voices/!v/m6", "espeak-ng-data/voices/!v/m7", "espeak-ng-data/voices/!v/m8",
    "espeak-ng-data/voices/!v/marcelo", "espeak-ng-data/voices/!v/Marco", "espeak-ng-data/voices/!v/Mario", "espeak-ng-data/voices/!v/max", "espeak-ng-data/lang/poz/mi", "espeak-ng-data/mi_dict", "espeak-ng-data/voices/!v/Michael", "espeak-ng-data/voices/!v/michel",
    "espeak-ng-data/voices/!v/miguel", "espeak-ng-data/voices/!v/Mike", "espeak-ng-data/voices/!v/mike2", "espeak-ng-data/lang/zls/mk", "espeak-ng-data/mk_dict", "espeak-ng-data/lang/dra/ml", "espeak-ng-data/ml_dict", "espeak-ng-data/lang/inc/mr",
    "espeak-ng-data/voices/!v/Mr serious", "espeak-ng-data/mr_dict", "espeak-ng-data/lang/poz/ms", "espeak-ng-data/ms_dict", "espeak-ng-data/lang/sem/mt", "espeak-ng-data/mt_dict", "espeak-ng-data/lang/miz/mto", "espeak-ng-data/mto_dict",
    "espeak-ng-data/lang/sit/my", "espeak-ng-data/my_dict", "espeak-ng-data/lang/gmq/nb", "espeak-ng-data/lang/azc/nci", "espeak-ng-data/nci_dict", "espeak-ng-data/lang/inc/ne", "espeak-ng-data/ne_dict", "espeak-ng-data/voices/!v/Nguyen",
    "espeak-ng-data/lang/gmw/nl", "espeak-ng-data/nl_dict", "espeak-ng-data/no_dict", "espeak-ng-data/lang/trk/nog", "espeak-ng-data/nog_dict", "espeak-ng-data/voices/!v/norbert", "espeak-ng-data/lang/cus/om", "espeak-ng-data/om_dict",
    "espeak-ng-data/lang/inc/or", "espeak-ng-data/or_dict", "espeak-ng-data/lang/inc/pa", "espeak-ng-data/pa_dict", "espeak-ng-data/voices/!v/pablo", "espeak-ng-data/lang/roa/pap", "espeak-ng-data/pap_dict", "espeak-ng-data/voices/!v/paul",
    "espeak-ng-data/voices/!v/pedro", "espeak-ng-data/phondata", "espeak-ng-data/phondata-manifest", "espeak-ng-data/phonindex", "espeak-ng-data/phontab", "espeak-ng-data/lang/art/piqd", "espeak-ng-data/piqd_dict", "espeak-ng-data/lang/zlw/pl",
    "espeak-ng-data/pl_dict", "espeak-ng-data/lang/roa/pt", "espeak-ng-data/lang/roa/pt-BR", "espeak-ng-data/pt_dict", "espeak-ng-data/lang/art/py", "espeak-ng-data/py_dict", "espeak-ng-data/lang/art/qdb", "espeak-ng-data/qdb_dict",
    "espeak-ng-data/lang/qu", "espeak-ng-data/qu_dict", "espeak-ng-data/lang/myn/quc", "espeak-ng-data/quc_dict", "espeak-ng-data/voices/!v/quincy", "espeak-ng-data/lang/art/qya", "espeak-ng-data/qya_dict", "espeak-ng-data/voices/!v/Reed",
    "espeak-ng-data/voices/!v/RicishayMax", "espeak-ng-data/voices/!v/RicishayMax2", "espeak-ng-data/voices/!v/RicishayMax3", "espeak-ng-data/lang/roa/ro", "espeak-ng-data/ro_dict", "espeak-ng-data/voices/!v/rob", "espeak-ng-data/voices/!v/robert", "espeak-ng-data/voices/!v/robosoft",
    "espeak-ng-data/voices/!v/robosoft2", "espeak-ng-data/voices/!v/robosoft3", "espeak-ng-data/voices/!v/robosoft4", "espeak-ng-data/voices/!v/robosoft5", "espeak-ng-data/voices/!v/robosoft6", "espeak-ng-data/voices/!v/robosoft7", "espeak-ng-data/voices/!v/robosoft8", "espeak-ng-data/lang/zle/ru",
    "espeak-ng-data/lang/zle/ru-cl", "espeak-ng-data/lang/zle/ru-LV", "espeak-ng-data/ru_dict", "espeak-ng-data/voices/!v/sandro", "espeak-ng-data/lang/inc/sd", "espeak-ng-data/sd_dict", "espeak-ng-data/voices/!v/shelby", "espeak-ng-data/lang/tai/shn",
    "espeak-ng-data/shn_dict", "espeak-ng-data/lang/inc/si", "espeak-ng-data/si_dict", "espeak-ng-data/lang/art/sjn", "espeak-ng-data/sjn_dict", "espeak-ng-data/lang/zlw/sk", "espeak-ng-data/sk_dict", "espeak-ng-data/lang/zls/sl",
    "espeak-ng-data/sl_dict", "espeak-ng-data/lang/urj/smj", "espeak-ng-data/smj_dict", "espeak-ng-data/lang/ine/sq", "espeak-ng-data/sq_dict", "espeak-ng-data/lang/zls/sr", "espeak-ng-data/sr_dict", "espeak-ng-data/voices/!v/steph",
    "espeak-ng-data/voices/!v/steph2", "espeak-ng-data/voices/!v/steph3", "espeak-ng-data/voices/!v/Storm", "espeak-ng-data/lang/gmq/sv", "espeak-ng-data/sv_dict", "espeak-ng-data/lang/bnt/sw", "espeak-ng-data/sw_dict", "espeak-ng-data/lang/dra/ta",
    "espeak-ng-data/ta_dict", "espeak-ng-data/lang/dra/te", "espeak-ng-data/te_dict", "espeak-ng-data/lang/tai/th", "espeak-ng-data/th_dict", "espeak-ng-data/lang/trk/tk", "espeak-ng-data/tk_dict", "espeak-ng-data/lang/bnt/tn",
    "espeak-ng-data/tn_dict", "espeak-ng-data/lang/trk/tr", "espeak-ng-data/tr_dict", "espeak-ng-data/voices/!v/travis", "espeak-ng-data/lang/trk/tt", "espeak-ng-data/tt_dict", "espeak-ng-data/voices/!v/Tweaky", "espeak-ng-data/lang/trk/ug",
    "espeak-ng-data/ug_dict", "espeak-ng-data/lang/zle/uk", "espeak-ng-data/uk_dict", "espeak-ng-data/voices/!v/UniRobot", "espeak-ng-data/lang/inc/ur", "espeak-ng-data/ur_dict", "espeak-ng-data/lang/trk/uz", "espeak-ng-data/uz_dict",
    "espeak-ng-data/lang/aav/vi", "espeak-ng-data/lang/aav/vi-VN-x-central", "espeak-ng-data/lang/aav/vi-VN-x-south", "espeak-ng-data/vi_dict", "espeak-ng-data/voices/!v/victor", "espeak-ng-data/voices/!v/whisper", "espeak-ng-data/voices/!v/whisperf", "espeak-ng-data/lang/sit/yue",
    "espeak-ng-data/lang/sit/yue-Latn-jyutping", "espeak-ng-data/yue_dict", "espeak-ng-data/voices/!v/zac",
];

/// matcha_tts_zh_en_20251010 的非 espeak 文件。
const MATCHA_ZH_EN_NON_ESPEAK_FILES: &[&str] = &[
    ".gitattributes", "README.md", "configuration.json", "lexicon.txt", "model-steps-6.onnx", "pytorch_model.bin", "pytorch_model_sherpa.bin", "test_sherpa_onnx.py",
    "tokens.txt", "vocab_tts.txt", "vocos-16khz-univ.onnx",
];

/// kokoro-int8-multi-lang-v1_1 的非 espeak 文件。
const KOKORO_NON_ESPEAK_FILES: &[&str] = &[
    "LICENSE", "README.md", "date-zh.fst", "dict/README.md", "dict/generate_user_dict.py", "dict/hmm_model.utf8", "dict/idf.utf8", "dict/jieba.dict.utf8",
    "dict/pos_dict/char_state_tab.utf8", "dict/pos_dict/prob_emit.utf8", "dict/pos_dict/prob_start.utf8", "dict/pos_dict/prob_trans.utf8", "dict/stop_words.utf8", "dict/user.dict.utf8", "lexicon-gb-en.txt", "lexicon-us-en.txt",
    "lexicon-zh.txt", "model.int8.onnx", "number-zh.fst", "phone-zh.fst", "tokens.txt", "voices.bin",
];

/// sherpa-onnx-zipvoice-distill-zh-en-emilia 的非 espeak 文件。
const ZIPVOICE_NON_ESPEAK_FILES: &[&str] = &[
    "1.wav", "fm_decoder.onnx", "fm_decoder_int8.onnx", "pinyin.raw", "prompt.wav", "text_encoder.onnx", "text_encoder_int8.onnx", "tokens.txt",
    "vocos_24khz.onnx",
];

/// matcha-icefall-zh-baker 的全部 19 个文件（不含 espeak-ng-data）。
const MATCHA_ZH_BAKER_FILES: &[&str] = &[
    "README.md", "date.fst", "dict/README.md", "dict/generate_user_dict.py", "dict/hmm_model.utf8", "dict/idf.utf8", "dict/jieba.dict.utf8", "dict/pos_dict/char_state_tab.utf8",
    "dict/pos_dict/prob_emit.utf8", "dict/pos_dict/prob_start.utf8", "dict/pos_dict/prob_trans.utf8", "dict/stop_words.utf8", "dict/user.dict.utf8", "lexicon.txt", "model-steps-3.onnx", "number.fst",
    "phone.fst", "tokens.txt", "vocos-22khz-univ.onnx",
];

fn concat_espeak(extra: &'static [&'static str]) -> Vec<&'static str> {
    let mut v = Vec::with_capacity(ESPEAK_NG_DATA_FILES.len() + extra.len());
    v.extend_from_slice(ESPEAK_NG_DATA_FILES);
    v.extend_from_slice(extra);
    v
}

/// 内置的 ModelScope 目录文件清单（相对子目录路径），API 不可用时作为静态回退。
/// 返回的路径需拼上 `modelscope_dir` 子目录才是仓库内完整路径。
pub fn static_subdir_files(id: &str) -> Option<Vec<&'static str>> {
    match id {
        "sherpa-matcha-zh" => Some(MATCHA_ZH_BAKER_FILES.to_vec()),
        "sherpa-matcha-zh-en" => Some(concat_espeak(MATCHA_ZH_EN_NON_ESPEAK_FILES)),
        "sherpa-kokoro-multi-lang" => Some(concat_espeak(KOKORO_NON_ESPEAK_FILES)),
        "sherpa-zipvoice-zh-en" => Some(concat_espeak(ZIPVOICE_NON_ESPEAK_FILES)),
        _ => None,
    }
}

pub fn expected_extension(entry: &AssetEntry) -> &'static str {
    if entry.download_url.ends_with(".tar.bz2") {
        "tar.bz2"
    } else if entry.download_url.ends_with(".zip") {
        "zip"
    } else if entry.download_url.ends_with(".json") {
        "json"
    } else if entry.download_url.ends_with(".onnx") {
        "onnx"
    } else if entry.download_url.ends_with(".pt") {
        "pt"
    } else if entry.download_url.ends_with(".7z") {
        "7z"
    } else {
        "bin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_uses_modelscope_for_non_sherpa_models() {
        let c = catalog();
        for asset in &c {
            if asset.kind == AssetKind::SherpaOnnx {
                // SherpaOnnx 必须能通过 ModelScope 目录（modelscope_dir）、HF 镜像逐文件
                // （hf_files），或 GitHub tar.bz2 整包（download_url 指向 release）下载。
                assert!(
                    asset.modelscope_dir.is_some()
                        || asset.hf_files.len() > 0
                        || asset.download_url.starts_with("https://github.com/k2-fsa/sherpa-onnx/releases/download/"),
                    "SherpaOnnx '{}' should have modelscope_dir, hf_files or a GitHub tar.bz2 download_url, got: {:?}",
                    asset.id,
                    asset.download_url
                );
                // 所有 SherpaOnnx 都应留有计划 A 失败的 GitHub 回退地址。
                assert!(
                    asset.github_fallback_url.is_some(),
                    "SherpaOnnx '{}' should have github_fallback_url",
                    asset.id
                );
                // 逐文件镜像方案里主模型必须落盘为本地加载器期望的名称。
                if !asset.hf_files.is_empty() {
                    assert!(asset.hf_files.contains_key("model.onnx"),
                        "SherpaOnnx '{}' hf_files must write primary model as 'model.onnx', keys: {:?}",
                        asset.id, asset.hf_files.keys().collect::<Vec<_>>());
                }
            } else {
                assert!(
                    asset.download_url.starts_with("https://www.modelscope.cn/"),
                    "Non-SherpaOnnx asset '{}' should use ModelScope, got: {}",
                    asset.id,
                    asset.download_url
                );
            }
        }
    }

    #[test]
    fn find_returns_some_for_registered_ids() {
        assert!(find("ling-v2").is_some());
        assert!(find("ling-v2-style").is_some());
        assert!(find("sherpa-matcha-zh").is_some());
        assert!(find("sherpa-matcha-zh-en").is_some());
        assert!(find("sherpa-kokoro-multi-lang").is_some());
        assert!(find("sherpa-zipvoice-zh-en").is_some());
        assert!(find("tsukuyomi").is_none());
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn expected_extension_handles_onnx_json_and_zip() {
        assert_eq!(expected_extension(&entry_with_url("a.onnx")), "onnx");
        assert_eq!(expected_extension(&entry_with_url("b.json")), "json");
        assert_eq!(expected_extension(&entry_with_url("c.zip")), "zip");
    }

    fn entry_with_url(url_suffix: &str) -> AssetEntry {
        AssetEntry {
            id: "tmp".into(),
            kind: AssetKind::Bert,
            display_name: "tmp".into(),
            language: "ja".into(),
            size_bytes: 0,
            download_url: format!("https://example.com/{url_suffix}"),
            source: "test".into(),
            voice_id: None,
            bundled_assets: vec![],
            sherpa_meta: None,
            hf_files: HashMap::new(),
            modelscope_dir: None,
            github_fallback_url: None,
        }
    }

    #[test]
    fn deberta_and_ling_v2_have_bundled_companions() {
        let deberta = find("deberta").unwrap();
        assert_eq!(deberta.bundled_assets, vec!["deberta-tokenizer"]);
        let ling = find("ling-v2").unwrap();
        assert_eq!(ling.bundled_assets, vec!["ling-v2-style"]);
    }

    #[test]
    fn sherpa_models_have_download_source_and_github_fallback() {
        let sherpa_ids = [
            "sherpa-matcha-zh",
            "sherpa-matcha-zh-en",
            "sherpa-kokoro-multi-lang",
            "sherpa-zipvoice-zh-en",
        ];
        for id in sherpa_ids {
            let entry = find(id).expect(id);
            let primary_source_ok = entry.modelscope_dir.is_some()
                || !entry.hf_files.is_empty()
                || entry.download_url.starts_with(
                    "https://github.com/k2-fsa/sherpa-onnx/releases/download/",
                );
            assert!(
                primary_source_ok,
                "{id} needs modelscope_dir, hf_files or GitHub tar.bz2 download_url"
            );
            assert!(entry.github_fallback_url.is_some(), "{id} should have github_fallback_url");
        }
    }

    #[test]
    fn every_modelscope_dir_has_a_static_fallback_list() {
        let mut mapped = 0;
        for entry in catalog() {
            if entry.kind != AssetKind::SherpaOnnx || entry.modelscope_dir.is_none() {
                continue;
            }
            let files = static_subdir_files(&entry.id).expect(&format!(
                "{} has modelscope_dir but no static_subdir_files",
                entry.id
            ));
            assert!(!files.is_empty(), "{} static list is empty", entry.id);
            for f in &files {
                assert!(!f.starts_with('/'), "{id}: absolute path in static list: {f}", id = entry.id);
                assert!(
                    !f.split('/').any(|seg| seg == ".."),
                    "{id}: path traversal in static list: {f}",
                    id = entry.id
                );
            }
            mapped += 1;
        }
        assert_eq!(mapped, 4);
        assert!(static_subdir_files("deberta").is_none());
        assert!(static_subdir_files("nonexistent").is_none());
    }
}
