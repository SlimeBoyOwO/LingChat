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

use super::burn_decoder::{self, BurnBaseDecoder};
use super::onnx_weights::read_initializers_from_bytes;

/// 把 burn `BurnBaseDecoder` 的权重写回基座 ONNX，写出新模型文件。
///
/// `read_decoder_param` 返回的值已是 ONNX 布局（MatMul/Gemm 转置已还原），按
/// decoder 语义名逐一覆写：命名张量用 `tensors` 里的名字，匿名 MatMul 常量用
/// `weight_map` 反查基座 initializer 名。最后按映射表把每个 decoder 目标写入
/// 对应基座 initializer。
pub fn export_tuned_model_burn(
    decoder: &BurnBaseDecoder,
    base_model: &Path,
    weight_map: &Path,
    out: &Path,
) -> Result<()> {
    export_tuned_model_burn_with_duration(decoder, base_model, weight_map, out, None)
}

/// 导出解码器，并可选地按风格簇校准时长预测器。
///
/// `duration_scale` 是目标语速相对基座预测的倍数（例如 1.1 表示慢 10%）。
/// 时长预测器输出对数秒，因此把 `ln(scale)` 加进
/// `model.encoder.proj_w.proj.bias` 即可整体缩放预测时长；不需要改图结构。
pub fn export_tuned_model_burn_with_duration(
    decoder: &BurnBaseDecoder,
    base_model: &Path,
    weight_map: &Path,
    out: &Path,
    duration_scale: Option<f32>,
) -> Result<()> {
    export_tuned_model_burn_with_adapters(decoder, &[], base_model, weight_map, out, duration_scale)
}

/// 导出解码器、编码器时长旁路与风格簇语速校准。
///
/// `encoder_weights` 是已经折叠好的编码器时长分支权重 `(initializer 名, ONNX
/// 布局 f32)`；为空表示不训练时长适配。它们与解码器一样原位覆写 initializer，
/// 不改变图结构。
pub fn export_tuned_model_burn_with_adapters(
    decoder: &BurnBaseDecoder,
    encoder_weights: &[(&str, Vec<f32>)],
    base_model: &Path,
    weight_map: &Path,
    out: &Path,
    duration_scale: Option<f32>,
) -> Result<()> {
    let bytes = std::fs::read(base_model)
        .with_context(|| format!("读取基座模型失败: {}", base_model.display()))?;
    let reverse = read_reverse_map(weight_map)?;

    // 训练出的风格 FiLM 不在基座 ONNX 里。先把它折进归一化仿射副本，
    // 后续按原语义名回读时就会自然带上风格调制。
    let folded = decoder.folded_for_export();
    let decoder = &folded;

    let mut export = bytes.to_vec();
    // 0) 编码器时长旁路：已折叠成基座权重，直接按 initializer 名覆写。
    for (name, flat) in encoder_weights {
        let mut data = Vec::with_capacity(flat.len() * 4);
        for value in flat {
            data.extend_from_slice(&value.to_le_bytes());
        }
        export = replace_initializer(&export, name, &data)
            .with_context(|| format!("替换编码器 initializer 失败: {name}"))?;
    }
    // 1) 命名解码器张量：语义名（`model.decoder.estimator.*`）即基座 initializer 名。
    for name in decoder_initializer_names(&export)? {
        if !name.starts_with("model.decoder.estimator.") {
            continue;
        }
        let Some(new_data) = read_decoder_param_bytes(decoder, &name) else {
            continue;
        };
        export = replace_initializer(&export, &name, &new_data)
            .with_context(|| format!("替换 initializer 失败: {name}"))?;
    }
    // 2) 匿名 MatMul 常量：语义名在 reverse 映射里，反查基座 `onnx::*` initializer。
    for (target, (src, _)) in &reverse {
        let Some(new_data) = read_decoder_param_bytes(decoder, target) else {
            continue;
        };
        export = replace_initializer(&export, src, &new_data)
            .with_context(|| format!("替换 initializer 失败: {src}"))?;
    }

    // 3) 按风格簇校准整体语速：只改时长投影的 bias，图结构不变。
    if let Some(scale) = duration_scale {
        if scale.is_finite() && scale > 0.0 && (scale - 1.0).abs() > 1e-4 {
            match apply_duration_scale(&export, scale) {
                Ok(updated) => export = updated,
                Err(error) => {
                    // 校准是增强项；换用其它基座导致参数名不同不应阻断导出。
                    tracing::warn!("[导出] 语速校准写入失败，忽略: {error:#}");
                },
            }
        }
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建输出目录失败: {}", parent.display()))?;
    }
    std::fs::write(out, &export).with_context(|| format!("写出模型失败: {}", out.display()))?;
    Ok(())
}

/// 把整体语速倍数写进时长预测器偏置。
///
/// `proj_w.proj` 是编码器最后的 1x1 卷积，输出对数秒。给它的 bias 加
/// `ln(scale)` 等价于把所有 token 的预测时长乘以 `scale`。
fn apply_duration_scale(export: &[u8], scale: f32) -> Result<Vec<u8>> {
    const NAME: &str = "model.encoder.proj_w.proj.bias";
    let tensors = read_initializers_from_bytes(export)?;
    let tensor = tensors
        .iter()
        .find(|tensor| tensor.name == NAME)
        .with_context(|| format!("基座里没有时长预测器偏置 {NAME}"))?;
    let shift = scale.ln();
    let mut data = Vec::with_capacity(tensor.data.len() * 4);
    for value in &tensor.data {
        data.extend_from_slice(&(value + shift).to_le_bytes());
    }
    replace_initializer(export, NAME, &data).with_context(|| format!("写入时长校准失败: {NAME}"))
}

/// 用 `read_decoder_param` 读 burn 解码器字段并转成 f32 小端字节。
///
/// `name` 是 decoder 语义名（`model.decoder.estimator.*` 或反查后的 `onnx::*`）。
fn read_decoder_param_bytes(decoder: &BurnBaseDecoder, name: &str) -> Option<Vec<u8>> {
    let flat = burn_decoder::read_decoder_param(decoder, name)?;
    let mut out = Vec::with_capacity(flat.len() * 4);
    for v in flat {
        out.extend_from_slice(&v.to_le_bytes());
    }
    Some(out)
}

/// 列出基座 ONNX 里属于解码器的 initializer 名，用于决定要覆写哪些张量。
///
/// 返回基座里所有 initializer 的名字（结构取自 graph 的 initializer 字段）。
/// 命名张量本身就带 `model.decoder.estimator.*` 前缀；匿名 MatMul 由映射表反查。
fn decoder_initializer_names(model_bytes: &[u8]) -> Result<Vec<String>> {
    let graph = find_submessage(model_bytes, 0, model_bytes.len(), 7)?
        .with_context(|| "ModelProto 里没有 graph(7)")?;
    let mut names = Vec::new();
    scan_fields(
        model_bytes,
        graph.start,
        graph.start + graph.len,
        |field, init_span| {
            if field != 5 {
                return true;
            }
            let mut name = None;
            let _ = scan_fields(
                model_bytes,
                init_span.start,
                init_span.start + init_span.len,
                |f, sp| {
                    if f == 8 {
                        name = Some(
                            std::str::from_utf8(&model_bytes[sp.start..sp.start + sp.len])
                                .unwrap_or("")
                                .to_string(),
                        );
                    }
                    true
                },
            );
            if let Some(n) = name {
                names.push(n);
            }
            true
        },
    )?;
    Ok(names)
}

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
    std::fs::write(out, &export).with_context(|| format!("写出模型失败: {}", out.display()))?;
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
fn resolve_initializer(name: &str, reverse: &HashMap<String, (String, bool)>) -> (String, bool) {
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
    scan_fields(
        buf,
        graph.start,
        graph.start + graph.len,
        |field, init_span| {
            if field != 5 {
                return true;
            }
            // 解析该 initializer：读名字与 raw_data。
            let mut name = None;
            let mut raw = None;
            // 内层找不到名字时忽略。
            let _ = scan_fields(
                buf,
                init_span.start,
                init_span.start + init_span.len,
                |f, sp| {
                    match f {
                        8 => {
                            name = Some(
                                std::str::from_utf8(&buf[sp.start..sp.start + sp.len])
                                    .unwrap_or(""),
                            );
                        },
                        9 => raw = Some(sp),
                        _ => {},
                    }
                    true
                },
            );
            if name.as_deref() == Some(tensor_name) {
                found = raw;
                false
            } else {
                true
            }
        },
    )?;
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
    use candle_core::{DType, Device, Tensor};
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

    /// 时长校准应只改动编码器时长投影 bias，且偏移量为 ln(scale)。
    #[test]
    fn duration_scale_shifts_encoder_bias() {
        let base = base_onnx();
        if !base.exists() {
            return;
        }
        let bytes = std::fs::read(&base).unwrap();
        let scale = 1.25f32;
        let out = apply_duration_scale(&bytes, scale).unwrap();
        let base_tensors = read_initializers_from_bytes(&bytes).unwrap();
        let out_tensors = read_initializers_from_bytes(&out).unwrap();
        let name = "model.encoder.proj_w.proj.bias";
        let before = base_tensors.iter().find(|t| t.name == name).unwrap();
        let after = out_tensors.iter().find(|t| t.name == name).unwrap();
        let shift = scale.ln();
        for (a, b) in before.data.iter().zip(&after.data) {
            assert!((a + shift - b).abs() < 1e-6, "时长 bias 偏移不正确");
        }
    }

    /// burn 导出应是无损恒等：把从基座加载的解码器写回，导出的解码器权重与基座一致。
    #[test]
    fn burn_export_is_identity_with_base_decoder() {
        use crate::ai_service::tts::finetune::burn_backend::train_device;
        use crate::ai_service::tts::finetune::burn_decoder::{
            BurnBaseDecoderConfig, from_onnx_tensors,
        };
        use crate::ai_service::tts::finetune::onnx_weights::read_initializers;

        let base = base_onnx();
        if !base.exists() {
            return;
        }
        let map_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/tts-training/decoder_weight_map.json");
        let device = train_device();
        let tensors = read_initializers(&base).unwrap();
        let decoder = from_onnx_tensors(
            &BurnBaseDecoderConfig::default(),
            &tensors,
            &map_path,
            &device,
        )
        .unwrap();

        let out = std::env::temp_dir().join("lingchat-export-burn-test.onnx");
        export_tuned_model_burn(&decoder, &base, &map_path, &out).unwrap();

        let base_tensors = read_initializers(&base).unwrap();
        let out_tensors = read_initializers(&out).unwrap();
        // 解码器命名张量应保持基座原值。
        for bt in &base_tensors {
            if !bt.name.starts_with("model.decoder.estimator.") {
                continue;
            }
            let ot = out_tensors.iter().find(|t| t.name == bt.name).unwrap();
            assert_eq!(ot.data, bt.data, "解码器张量不应被改动: {}", bt.name);
        }
        let _ = std::fs::remove_file(&out);
    }
}
