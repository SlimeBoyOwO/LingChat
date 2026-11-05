//! 把微调后的解码器权重写回基座 ONNX，另存为自家模型。
//!
//! 解码器是基座里唯一被微调的部分，且权重形状与基座一致，因此可以把训练后
//! 的 f32 值原位覆写进基座 `ModelProto` 同名 `initializer` 的 `raw_data` 字节段，
//! 其余 graph 结构原样保留、长度不变。这样导出的模型与基座同结构，现有 ort
//! 采样推理路径可直接加载，无需重搭计算图。
//!
//! 实现上不依赖 protobuf 库：手工扫描字段、以绝对偏移做定位与替换。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use candle_nn::VarMap;

/// 把训练后的解码器权重覆写进基座模型，写出新模型文件。
///
/// 解码器是基座里唯一被微调的部分，且权重形状与基座一致，因此可以把训练后
/// 的 f32 值原位覆写进基座里对应的 `initializer`，其余 graph 结构原样保留、
/// 长度不变，导出的模型与基座同结构，现有 ort 采样推理路径可直接加载。
///
/// `weight_map` 提供 `onnx::*` 匿名 initializer 与语义权重名的对应关系
/// （由 `decoder_weight_map.json` 给出，其中 MatMul 权重存 [in,out]、candle 投影
/// 存 [out,in]，替换时需转置）。命名张量（卷积/归一化等）直接同名对应，无需映射。
pub fn export_tuned_model(
    decoder_vm: &VarMap,
    base_model: &Path,
    weight_map: &Path,
    out: &Path,
) -> Result<()> {
    let bytes = std::fs::read(base_model)
        .with_context(|| format!("读取基座模型失败: {}", base_model.display()))?;
    let reverse = read_reverse_map(weight_map)?;

    let mut export = bytes.to_vec();
    for name in var_names(decoder_vm)? {
        let (src, transposed) = resolve_initializer(&name, &reverse);
        let new_bytes = var_bytes(decoder_vm, &name, transposed)?;
        export = replace_initializer(&export, &src, &new_bytes)
            .with_context(|| format!("替换 initializer 失败: {src}"))?;
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建输出目录失败: {}", parent.display()))?;
    }
    std::fs::write(out, &export)
        .with_context(|| format!("写出模型失败: {}", out.display()))?;
    Ok(())
}

/// 从权重映射表构建「语义名 → (基座 initializer 名, 是否需转置)」的反查表。
///
/// 映射表 `src(onnx::*)` → `target(语义)`；导出需要按 target 找 src。命名张量
/// 不在表里，回退为同名。
fn read_reverse_map(map_path: &Path) -> Result<HashMap<String, (String, bool)>> {
    let bytes = std::fs::read(map_path)
        .with_context(|| format!("读取权重映射表失败: {}", map_path.display()))?;
    let map: HashMap<String, serde_json::Value> = serde_json::from_slice(&bytes)?;
    let mut reverse = HashMap::new();
    for (src, info) in &map {
        let Some(target) = info.get("target").and_then(|v| v.as_str()) else {
            continue;
        };
        // 只关心解码器目标；编码器匿名归一化不在此处导出。
        if !target.starts_with("model.decoder.") {
            continue;
        }
        let target = target.replace('/', ".");
        let transposed = src.starts_with("onnx::MatMul_");
        reverse.insert(target, (src.clone(), transposed));
    }
    Ok(reverse)
}

/// 解析一个 varmap 语义名对应的基座 initializer 名与转置标记。
fn resolve_initializer(
    name: &str,
    reverse: &HashMap<String, (String, bool)>,
) -> (String, bool) {
    if let Some((src, transposed)) = reverse.get(name) {
        return (src.clone(), *transposed);
    }
    (name.to_string(), false)
}

/// 解码器 varmap 里登记的所有参数名。
fn var_names(decoder_vm: &VarMap) -> Result<Vec<String>> {
    let data = decoder_vm
        .data()
        .lock()
        .map_err(|_| anyhow::anyhow!("VarMap 锁已毒化"))?;
    Ok(data.keys().cloned().collect())
}

/// 取某个 Var 的扁平 f32 小端字节。`transposed` 为真表示是 MatMul 投影权重，
/// candle 存 [out,in]、ONNX 存 [in,out]，导出前需转置。
fn var_bytes(decoder_vm: &VarMap, name: &str, transposed: bool) -> Result<Vec<u8>> {
    let data = decoder_vm
        .data()
        .lock()
        .map_err(|_| anyhow::anyhow!("VarMap 锁已毒化"))?;
    let var = data
        .get(name)
        .with_context(|| format!("VarMap 里找不到 {name}"))?;
    let tensor = if transposed {
        var.as_tensor().t()?.contiguous()?
    } else {
        var.as_tensor().clone()
    };
    let flat = tensor.flatten_all()?.to_vec1::<f32>()?;
    drop(data);
    let mut out = Vec::with_capacity(flat.len() * 4);
    for v in flat {
        out.extend_from_slice(&v.to_le_bytes());
    }
    Ok(out)
}

/// 在模型字节流里定位并替换某 `initializer` 的 `raw_data`(9) 字节段。
///
/// 新数据长度必须与目标一致，否则拒绝（防止破坏文件结构）。
fn replace_initializer(model_bytes: &[u8], tensor_name: &str, new_data: &[u8]) -> Result<Vec<u8>> {
    let graph = find_submessage(model_bytes, 0, model_bytes.len(), 7)?
        .with_context(|| "ModelProto 里没有 graph(7)")?;
    let slot = find_init_raw_data(model_bytes, graph, tensor_name)?
        .with_context(|| format!("基座里没有 initializer {tensor_name}"))?;
    if slot.len != new_data.len() {
        bail!(
            "initializer {tensor_name} 数据长度不符: 原 {} 新 {}",
            slot.len,
            new_data.len()
        );
    }
    let mut out = model_bytes.to_vec();
    out[slot.start..slot.start + slot.len].copy_from_slice(new_data);
    Ok(out)
}

/// 在当前 proto 顶层区间内找 `target` 字段（length-delimited）的绝对载荷区间。
fn find_submessage(buf: &[u8], start: usize, end: usize, target: u32) -> Result<Option<Span>> {
    let mut found = None;
    scan_fields(buf, start, end, |field, span| {
        if field == target {
            found = Some(span);
            false
        } else {
            true
        }
    })?;
    Ok(found)
}

/// 在 graph 区间里找指定名字 initializer 的 raw_data 绝对载荷区间。
fn find_init_raw_data(buf: &[u8], graph: Span, tensor_name: &str) -> Result<Option<Span>> {
    let mut found = None;
    scan_fields(buf, graph.start, graph.start + graph.len, |field, init_span| {
        if field != 5 {
            return true;
        }
        // 解析该 initializer：读名字与 raw_data。
        let mut name = None;
        let mut raw = None;
        // 内层找不到名字时忽略。
        let _ = scan_fields(buf, init_span.start, init_span.start + init_span.len, |f, sp| {
            match f {
                8 => {
                    name = Some(std::str::from_utf8(&buf[sp.start..sp.start + sp.len]).unwrap_or(""));
                },
                9 => raw = Some(sp),
                _ => {},
            }
            true
        });
        if name.as_deref() == Some(tensor_name) {
            found = raw;
            false
        } else {
            true
        }
    })?;
    Ok(found)
}

/// 遍历一个 proto 区间内的所有字段，回调收到 (字段号, 载荷绝对Span)。
///
/// 回调返回 false 则提前终止。length-delimited 载荷的 Span 覆盖值字节，不含
/// key 与长度前缀。
fn scan_fields(
    buf: &[u8],
    start: usize,
    end: usize,
    mut visit: impl FnMut(u32, Span) -> bool,
) -> Result<()> {
    let mut pos = start;
    while pos < end {
        let (field, wire) = read_key(buf, &mut pos)?;
        match wire {
            0 => {
                read_varint(buf, &mut pos)?;
            },
            1 => {
                pos += 8;
            },
            2 => {
                let len = read_varint(buf, &mut pos)? as usize;
                let span = Span { start: pos, len };
                pos += len;
                let continue_ = visit(field, span);
                if !continue_ {
                    break;
                }
            },
            5 => {
                pos += 4;
            },
            other => bail!("不支持的线格式: {other}"),
        }
    }
    Ok(())
}

/// 读取一个字段 key，返回 (字段号, 线类型)。
fn read_key(buf: &[u8], pos: &mut usize) -> Result<(u32, u8)> {
    let key = read_varint(buf, pos)?;
    Ok(((key >> 3) as u32, (key & 7) as u8))
}

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        if *pos >= buf.len() {
            bail!("varint 越界");
        }
        let byte = buf[*pos];
        *pos += 1;
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

/// 一个反字节区间的绝对偏移（左闭右开，按长度描述）。
#[derive(Debug, Clone, Copy)]
struct Span {
    start: usize,
    len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, DType, Tensor};
    use candle_nn::{VarBuilder, VarMap};

    fn base_onnx() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx")
    }

    #[test]
    fn find_initializer_slot_locates_decoder_weight() {
        let path = base_onnx();
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(&path).unwrap();
        let graph = find_submessage(&bytes, 0, bytes.len(), 7)
            .unwrap()
            .expect("应找到 graph");
        let name = "model.decoder.estimator.up_blocks.0.2.conv.weight";
        let slot = find_init_raw_data(&bytes, graph, name)
            .unwrap()
            .expect("应找到转置卷积权重");
        // 基座转置卷积权重形状 [256,256,4]，f32 字节 = 256*256*4*4。
        assert_eq!(slot.len, 256 * 256 * 4 * 4);
    }

    #[test]
    fn export_tuned_model_writes_decoder_and_keeps_encoder() {
        use crate::ai_service::tts::finetune::read_initializers;
        use crate::ai_service::tts::finetune::{BaseDecoder, export_tuned_model};

        let base = base_onnx();
        if !base.exists() {
            return;
        }
        let device = Device::Cpu;
        let mut var_map = VarMap::new();
        let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
        let _decoder = BaseDecoder::new(&Default::default(), vb).unwrap();

        // 把转置卷积权重覆写为一个确定常量，便于核对导出结果。
        let wname = "model.decoder.estimator.up_blocks.0.2.conv.weight";
        let new_weight = vec![0.5f32; 256 * 256 * 4];
        var_map
            .set_one(
                wname,
                &Tensor::from_vec(new_weight.clone(), (256, 256, 4), &device).unwrap(),
            )
            .unwrap();

        let out = std::env::temp_dir().join("lingchat-export-test.onnx");
        let map_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/tts-training/decoder_weight_map.json");
        export_tuned_model(&var_map, &base, &map_path, &out).unwrap();

        let tensors = read_initializers(&out).unwrap();
        let conv = tensors
            .iter()
            .find(|t| t.name == wname)
            .expect("导出后应有转置卷积权重");
        assert_eq!(conv.data, new_weight, "解码器权重应被覆写");

        // 匿名 MatMul 权重（to_q）应经映射表写入，且布局转置回 ONNX 的 [in,out]。
        let to_q_name = "model.decoder.estimator.down_blocks.0.1.0.attn1.to_q.weight";
        let src = "onnx::MatMul_6149";
        let exported = tensors
            .iter()
            .find(|t| t.name == src)
            .expect("应导出 to_q 的匿名 initializer");
        // candle [out,in] 转置为 ONNX [in,out]，与原 varmap 值一致。
        let var_t = var_map
            .data()
            .lock()
            .unwrap()
            .get(to_q_name)
            .unwrap()
            .as_tensor()
            .t()
            .unwrap()
            .contiguous()
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        assert_eq!(exported.data, var_t, "匿名 MatMul 权重应转置后导出");

        // 编码器权重应保持基座原值。
        let enc = "model.encoder.emb.weight";
        let base_tensors = read_initializers(&base).unwrap();
        let base_enc = base_tensors.iter().find(|t| t.name == enc).unwrap();
        let out_enc = tensors.iter().find(|t| t.name == enc).unwrap();
        assert_eq!(out_enc.data, base_enc.data, "编码器权重不应被改动");

        let _ = std::fs::remove_file(&out);
    }
}