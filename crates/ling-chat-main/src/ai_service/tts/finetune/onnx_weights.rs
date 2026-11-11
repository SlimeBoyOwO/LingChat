//! 读取 ONNX 里的 f32 权重张量：直接按 protobuf 线格式解析，不引入依赖。

use std::path::Path;

use anyhow::{Context, Result, bail};

/// 一个权重张量。
#[derive(Debug, Clone)]
pub struct OnnxTensor {
    pub name: String,
    pub dims: Vec<usize>,
    /// 按行主序展开的 f32 数据。
    pub data: Vec<f32>,
}

impl OnnxTensor {
    /// 元素个数。
    pub fn params(&self) -> usize {
        self.dims.iter().product()
    }
}

/// 读取模型里所有 f32 权重张量。
pub fn read_initializers(path: &Path) -> Result<Vec<OnnxTensor>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("读取 ONNX 失败: {}", path.display()))?;
    read_initializers_from_bytes(&bytes)
        .with_context(|| format!("解析 ONNX 失败: {}", path.display()))
}

/// 从字节流解析。
pub fn read_initializers_from_bytes(bytes: &[u8]) -> Result<Vec<OnnxTensor>> {
    let graph = find_submessage(bytes, 7)?.context("ONNX 里没有 graph")?;
    let mut tensors = Vec::new();

    let mut reader = Reader::new(graph);
    while !reader.done() {
        let (field, wire) = reader.key()?;
        match (field, wire) {
            (5, 2) => {
                let raw = reader.bytes()?;
                if let Some(tensor) = parse_tensor(raw)? {
                    tensors.push(tensor);
                }
            },
            _ => reader.skip(wire)?,
        }
    }

    if tensors.is_empty() {
        bail!("ONNX 里没有可用的 f32 权重");
    }
    Ok(tensors)
}

/// 找到顶层消息里某个字段的子消息。
fn find_submessage<'a>(bytes: &'a [u8], target: u32) -> Result<Option<&'a [u8]>> {
    let mut reader = Reader::new(bytes);
    while !reader.done() {
        let (field, wire) = reader.key()?;
        if field == target && wire == 2 {
            return Ok(Some(reader.bytes()?));
        }
        reader.skip(wire)?;
    }
    Ok(None)
}

fn parse_tensor(bytes: &[u8]) -> Result<Option<OnnxTensor>> {
    let mut reader = Reader::new(bytes);
    let mut dims = Vec::new();
    let mut data_type = 0u64;
    let mut name = String::new();
    let mut raw_data: Option<&[u8]> = None;
    let mut float_data = Vec::new();

    while !reader.done() {
        let (field, wire) = reader.key()?;
        match (field, wire) {
            (1, 0) => dims.push(reader.varint()? as usize),
            (1, 2) => {
                let packed = reader.bytes()?;
                let mut inner = Reader::new(packed);
                while !inner.done() {
                    dims.push(inner.varint()? as usize);
                }
            },
            (2, 0) => data_type = reader.varint()?,
            (4, 2) => {
                let packed = reader.bytes()?;
                if packed.len() % 4 != 0 {
                    bail!("float_data 长度不是 4 的倍数");
                }
                for chunk in packed.chunks_exact(4) {
                    float_data.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
                }
            },
            (8, 2) => {
                name = String::from_utf8_lossy(reader.bytes()?).into_owned();
            },
            (9, 2) => raw_data = Some(reader.bytes()?),
            _ => reader.skip(wire)?,
        }
    }

    // 只接受 FLOAT；其他类型微调用不到，跳过。
    if data_type != 1 {
        return Ok(None);
    }

    let data = if let Some(raw) = raw_data {
        if raw.len() % 4 != 0 {
            bail!("raw_data 长度不是 4 的倍数: {name}");
        }
        raw.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    } else {
        float_data
    };

    Ok(Some(OnnxTensor { name, dims, data }))
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn varint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            if self.pos >= self.buf.len() {
                bail!("varint 越界");
            }
            let byte = self.buf[self.pos];
            self.pos += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
            if shift >= 64 {
                bail!("varint 过长");
            }
        }
    }

    fn key(&mut self) -> Result<(u32, u8)> {
        let key = self.varint()?;
        Ok(((key >> 3) as u32, (key & 7) as u8))
    }

    fn bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.varint()? as usize;
        let end = self.pos.checked_add(len).context("长度溢出")?;
        if end > self.buf.len() {
            bail!("字段长度越界");
        }
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn skip(&mut self, wire: u8) -> Result<()> {
        match wire {
            0 => {
                self.varint()?;
            },
            1 => {
                if self.pos + 8 > self.buf.len() {
                    bail!("fixed64 越界");
                }
                self.pos += 8;
            },
            2 => {
                self.bytes()?;
            },
            5 => {
                if self.pos + 4 > self.buf.len() {
                    bail!("fixed32 越界");
                }
                self.pos += 4;
            },
            other => bail!("不支持的线格式: {other}"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_base_model_weights() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx");
        if !path.exists() {
            return;
        }

        let tensors = read_initializers(&path).unwrap();
        assert_eq!(tensors.len(), 305);

        let params: usize = tensors.iter().map(OnnxTensor::params).sum();
        assert!(
            params > 18_000_000 && params < 19_000_000,
            "params {params}"
        );

        let embedding = tensors
            .iter()
            .find(|tensor| tensor.name == "model.encoder.emb.weight")
            .expect("缺少词嵌入");
        assert_eq!(embedding.dims, vec![2069, 192]);
        assert_eq!(embedding.data.len(), 2069 * 192);
    }
}
