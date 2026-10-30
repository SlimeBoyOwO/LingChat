//! 直接从 ONNX 模型里读 metadata（`n_speakers` / `speaker_names` 等）。
//!
//! sherpa-onnx 加载模型时读的就是这些键（见上游 `offline-tts-vits-model.cc` 的
//! `SHERPA_ONNX_READ_META_DATA(meta_data_.num_speakers, "n_speakers")`），它们写在
//! ONNX `ModelProto.metadata_props`（field 14）里。这里按 protobuf 的 wire format
//! 走一遍顶层字段，跳过 graph 等大块数据：既不用 onnxruntime，也不把模型读进内存，
//! 所以「模型有多少个音色」是毫秒级查询 —— 设置页只是想列个列表，没必要为它加载
//! 几百 MB 的模型（kokoro 加载一次要一两秒，安卓上更慢）。
//!
//! 解析失败一律报错，让调用方回退到「只有四个语义档」；不能编出一个音色列表。

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

/// ONNX `ModelProto.metadata_props` 的 field number。
const METADATA_PROPS: u64 = 14;

/// 读取 ONNX 模型 `metadata_props` 里的全部键值。
pub(super) fn read_onnx_metadata(path: &Path) -> Result<BTreeMap<String, String>> {
    let file = File::open(path).with_context(|| format!("打开模型文件失败: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut meta = BTreeMap::new();

    while let Some(tag) = read_varint(&mut reader)? {
        let field = tag >> 3;
        match (tag & 0x7) as u8 {
            // varint：ir_version / model_version 等
            0 => {
                if read_varint(&mut reader)?.is_none() {
                    bail!("模型文件被截断");
                }
            },
            // 64 位定长（double / fixed64）
            1 => skip_bytes(&mut reader, 8)?,
            // length-delimited：graph、metadata_props、字符串字段等
            2 => {
                let len = read_varint(&mut reader)?.ok_or_else(|| anyhow!("模型文件被截断"))?;
                if field == METADATA_PROPS {
                    let mut buf = Vec::new();
                    reader
                        .by_ref()
                        .take(len)
                        .read_to_end(&mut buf)
                        .context("读取模型 metadata 失败")?;
                    if buf.len() as u64 != len {
                        bail!("模型文件被截断");
                    }
                    let (key, value) = parse_metadata_entry(&buf)?;
                    meta.insert(key, value);
                } else {
                    skip_bytes(&mut reader, len)?;
                }
            },
            // 32 位定长（float / fixed32）
            5 => skip_bytes(&mut reader, 4)?,
            wire => bail!("模型文件里有不支持的 protobuf wire type: {wire}"),
        }
    }

    Ok(meta)
}

/// 从 metadata 里解析说话人数量与名字（下标即 sid）。
///
/// - `n_speakers`：sherpa 自己也读这个键，缺省是 1。
/// - `speaker_names`：逗号分隔，顺序就是 sid（kokoro 的 `af_maple,zf_001,…`）。
/// - `id2speaker`：老模型的写法，`0->a,1->b`，只在没有 `speaker_names` 时用。
pub(super) fn speakers_from_metadata(meta: &BTreeMap<String, String>) -> (usize, Vec<String>) {
    let mut names = metadata_names(meta);
    let num_speakers = meta
        .get("n_speakers")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .or_else(|| (!names.is_empty()).then_some(names.len()))
        .unwrap_or(1);

    // 名字比人数少时补空串，多了就截掉：前端按「没有名字就只显示 sid」处理。
    names.resize(num_speakers, String::new());
    names.truncate(num_speakers);
    (num_speakers, names)
}

fn metadata_names(meta: &BTreeMap<String, String>) -> Vec<String> {
    if let Some(list) = meta.get("speaker_names") {
        let names: Vec<String> = list
            .split(',')
            .map(|name| name.trim().to_string())
            .collect();
        if !names.is_empty() {
            return names;
        }
    }

    if let Some(pairs) = meta.get("id2speaker") {
        let mut names: Vec<(usize, String)> = Vec::new();
        for pair in pairs.split(',') {
            let Some((sid, name)) = pair.split_once("->") else {
                continue;
            };
            let Ok(sid) = sid.trim().parse::<usize>() else {
                continue;
            };
            names.push((sid, name.trim().to_string()));
        }
        if !names.is_empty() {
            let max = names.iter().map(|(sid, _)| *sid).max().unwrap_or(0);
            let mut ordered = vec![String::new(); max + 1];
            for (sid, name) in names {
                ordered[sid] = name;
            }
            return ordered;
        }
    }

    Vec::new()
}

fn read_varint<R: Read>(reader: &mut R) -> Result<Option<u64>> {
    let mut value: u64 = 0;
    let mut shift = 0;
    for _ in 0..10 {
        let mut byte = [0u8; 1];
        if reader.read(&mut byte)? == 0 {
            return Ok(None);
        }
        value |= u64::from(byte[0] & 0x7f) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(Some(value));
        }
        shift += 7;
    }
    bail!("模型文件里的 protobuf varint 过长")
}

fn skip_bytes<R: Read>(reader: &mut R, len: u64) -> Result<()> {
    let skipped = std::io::copy(&mut reader.by_ref().take(len), &mut std::io::sink())?;
    if skipped != len {
        bail!("模型文件被截断");
    }
    Ok(())
}

/// 解析一个 `StringStringEntryProto`：key = field 1，value = field 2。
fn parse_metadata_entry(buf: &[u8]) -> Result<(String, String)> {
    let mut key = String::new();
    let mut value = String::new();
    let mut i = 0;
    while i < buf.len() {
        let (tag, next) = varint_at(buf, i)?;
        i = next;
        if (tag & 0x7) != 2 {
            bail!("模型 metadata 字段格式异常");
        }
        let (len, next) = varint_at(buf, i)?;
        i = next;
        let end = i
            .checked_add(len as usize)
            .filter(|end| *end <= buf.len())
            .ok_or_else(|| anyhow!("模型 metadata 被截断"))?;
        let text = String::from_utf8_lossy(&buf[i..end]).into_owned();
        match tag >> 3 {
            1 => key = text,
            2 => value = text,
            _ => {},
        }
        i = end;
    }
    Ok((key, value))
}

fn varint_at(buf: &[u8], start: usize) -> Result<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0;
    let mut i = start;
    for _ in 0..10 {
        let byte = *buf.get(i).ok_or_else(|| anyhow!("模型 metadata 被截断"))?;
        i += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, i));
        }
        shift += 7;
    }
    bail!("模型 metadata 里的 protobuf varint 过长")
}

/// 造一个只含 metadata 的假 ONNX 文件内容，供测试走完整的「找文件 → 解析」链路。
#[cfg(test)]
pub(super) fn fake_onnx(entries: &[(&str, &str)]) -> Vec<u8> {
    fn varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                return out;
            }
        }
    }

    fn field_bytes(field: u64, data: &[u8]) -> Vec<u8> {
        let mut out = varint((field << 3) | 2);
        out.extend(varint(data.len() as u64));
        out.extend(data);
        out
    }

    let mut model = Vec::new();
    // ir_version = 8，让解析器顺带走一遍 varint 字段。
    model.extend(varint(1 << 3));
    model.extend(varint(8));
    // graph 占一大块，确认会被跳过而不是读进内存。
    model.extend(field_bytes(7, &vec![0u8; 64 * 1024]));
    for (key, value) in entries {
        let mut entry = field_bytes(1, key.as_bytes());
        entry.extend(field_bytes(2, value.as_bytes()));
        model.extend(field_bytes(14, &entry));
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_fake(dir: &Path, entries: &[(&str, &str)]) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).expect("建临时目录失败");
        let path = dir.join("model.onnx");
        let mut file = File::create(&path).expect("建假模型失败");
        file.write_all(&fake_onnx(entries)).expect("写假模型失败");
        path
    }

    #[test]
    fn reads_metadata_and_skips_graph() {
        let dir = std::env::temp_dir().join(format!("lingchat-onnx-meta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = write_fake(
            &dir,
            &[
                ("n_speakers", "3"),
                ("speaker_names", "af_maple, zf_001,zm_009"),
                ("model_type", "kokoro"),
            ],
        );

        let meta = read_onnx_metadata(&path).expect("解析 metadata 失败");
        assert_eq!(meta.get("n_speakers").map(String::as_str), Some("3"));
        assert_eq!(meta.get("model_type").map(String::as_str), Some("kokoro"));
        assert_eq!(
            meta.get("speaker_names").map(String::as_str),
            Some("af_maple, zf_001,zm_009")
        );

        let (num, names) = speakers_from_metadata(&meta);
        assert_eq!(num, 3);
        assert_eq!(names, ["af_maple", "zf_001", "zm_009"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_speaker_names_and_id2speaker() {
        let mut meta = BTreeMap::new();
        meta.insert("n_speakers".to_string(), "3".to_string());
        meta.insert("speaker_names".to_string(), "a,b,c".to_string());
        assert_eq!(
            speakers_from_metadata(&meta),
            (3, vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );

        // 老模型只有 id2speaker，等价于 speaker_names。
        let mut meta = BTreeMap::new();
        meta.insert("n_speakers".to_string(), "3".to_string());
        meta.insert(
            "id2speaker".to_string(),
            "0->af_maple,2->zm_009".to_string(),
        );
        let (num, names) = speakers_from_metadata(&meta);
        assert_eq!(num, 3);
        assert_eq!(names, ["af_maple", "", "zm_009"]);

        // 没有 metadata 的单说话人模型。
        let (num, names) = speakers_from_metadata(&BTreeMap::new());
        assert_eq!((num, names.len()), (1, 1));
        assert!(names[0].is_empty());

        // 名字比 n_speakers 少/多都要对齐。
        let mut meta = BTreeMap::new();
        meta.insert("n_speakers".to_string(), "2".to_string());
        meta.insert("speaker_names".to_string(), "a,b,c".to_string());
        assert_eq!(
            speakers_from_metadata(&meta),
            (2, vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn truncated_model_is_an_error() {
        let dir = std::env::temp_dir().join(format!("lingchat-onnx-trunc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录失败");
        let path = dir.join("model.onnx");
        // field 7 + 声称 100 字节，但只有 3 字节。
        std::fs::write(&path, [0x3a, 0x64, 0x01, 0x02, 0x03]).expect("写假模型失败");
        assert!(read_onnx_metadata(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
