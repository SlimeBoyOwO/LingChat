//! 用 burn 实现的基座文本编码器，与 candle 版逐层对齐。
//!
//! 网络结构：词嵌入、三层 k=5 的 prenet 卷积（各带沿通道维的 AffineNorm）、
//! 六层 transformer 块（卷积投影自注意力 + k=3 前馈卷积，各带残差与归一化），
//! 末尾分出 mel 先验与对数时长两支。权重名约定与 candle 版一致。

use burn::module::Param;
use burn::nn::conv::{Conv1d, Conv1dConfig};
use burn::nn::PaddingConfig1d;
use burn::prelude::*;
use burn::tensor::activation::{relu, softmax};

use super::onnx_weights::OnnxTensor;

/// 编码器超参数（burn 版，字段与 candle 版对应）。
#[derive(Debug, Clone)]
pub struct BurnBaseEncoderConfig {
    /// 词表大小。
    pub n_vocab: usize,
    /// 隐层维度。
    pub hidden: usize,
    /// Transformer 层数。
    pub layers: usize,
    /// 前馈隐层维度。
    pub ffn_hidden: usize,
    /// mel 通道数。
    pub n_mels: usize,
    /// 时长预测器隐层维度。
    pub duration_hidden: usize,
}

impl Default for BurnBaseEncoderConfig {
    fn default() -> Self {
        Self {
            n_vocab: 2069,
            hidden: 192,
            layers: 6,
            ffn_hidden: 768,
            n_mels: 80,
            duration_hidden: 256,
        }
    }
}

/// 沿通道维归一化并做逐通道仿射（等价 candle 的 AffineNorm）。
#[derive(Module, Debug)]
struct AffineNorm {
    weight: Param<Tensor<3>>,
    bias: Param<Tensor<3>>,
}

impl AffineNorm {
    fn new(channels: usize, device: &Device) -> Self {
        Self {
            weight: Param::from_tensor(Tensor::<3>::ones([1, channels, 1], device)),
            bias: Param::from_tensor(Tensor::<3>::zeros([1, channels, 1], device)),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let mean = xs.clone().mean_dims(&[1]);
        let centered = xs.clone() - mean;
        let variance = centered.clone().powf_scalar(2.0).mean_dims(&[1]);
        let normed = centered / (variance + 1e-5).sqrt();
        normed * self.weight.val() + self.bias.val()
    }
}

/// 自注意力：四个 1x1 卷积投影，分 2 头在帧维上做缩放点积。
#[derive(Module, Debug)]
struct Attention {
    query: Conv1d,
    key: Conv1d,
    value: Conv1d,
    output: Conv1d,
}

impl Attention {
    fn new(hidden: usize, device: &Device) -> Self {
        let config = Conv1dConfig::new(hidden, hidden, 1);
        Self {
            query: config.clone().init(device),
            key: config.clone().init(device),
            value: config.clone().init(device),
            output: config.init(device),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, hidden, frames] = xs.dims();
        let heads = 2;
        let head_dim = hidden / heads;

        let query = self.query.forward(xs.clone()).reshape([batch, heads, head_dim, frames]);
        let key = self.key.forward(xs.clone()).reshape([batch, heads, head_dim, frames]);
        let value = self.value.forward(xs).reshape([batch, heads, head_dim, frames]);

        // 注意力分数 [b,h,f,f]：q[b,h,f,d] @ k[b,h,f,d]^T。
        let q = query.permute([0, 1, 3, 2]);
        let k = key.permute([0, 1, 3, 2]);
        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = softmax(q.matmul(k.transpose()) * scale, 2);
        let v = value.permute([0, 1, 3, 2]);
        let context = scores.matmul(v).permute([0, 1, 3, 2]);
        let context = context.reshape([batch, hidden, frames]);
        self.output.forward(context)
    }
}

/// 前馈：两个 k=3 卷积 + relu。
#[derive(Module, Debug)]
struct FeedForward {
    conv1: Conv1d,
    conv2: Conv1d,
}

impl FeedForward {
    fn new(hidden: usize, ffn_hidden: usize, device: &Device) -> Self {
        let padded = |channels_in, channels_out| {
            Conv1dConfig::new(channels_in, channels_out, 3).with_padding(PaddingConfig1d::Explicit(1, 1))
        };
        Self {
            conv1: padded(hidden, ffn_hidden).init(device),
            conv2: padded(ffn_hidden, hidden).init(device),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let hidden = relu(self.conv1.forward(xs));
        self.conv2.forward(hidden)
    }
}

/// 文本编码器（burn 版）。
#[derive(Module, Debug)]
pub struct BurnBaseEncoder {
    embedding: burn::nn::Embedding,
    prenet: Vec<Conv1d>,
    prenet_proj: Conv1d,
    prenet_norms: Vec<AffineNorm>,
    block_norms: Vec<(AffineNorm, AffineNorm)>,
    attentions: Vec<Attention>,
    feed_forwards: Vec<FeedForward>,
    proj_m: Conv1d,
    duration_conv1: Conv1d,
    duration_conv2: Conv1d,
    duration_proj: Conv1d,
}

impl BurnBaseEncoder {
    /// 随机初始化（权重不来自基座，用于验证结构）。
    pub fn init(config: &BurnBaseEncoderConfig, device: &Device) -> Self {
        let embedding = burn::nn::EmbeddingConfig::new(config.n_vocab, config.hidden).init(device);

        let mut prenet = Vec::with_capacity(3);
        let padded = Conv1dConfig::new(config.hidden, config.hidden, 5)
            .with_padding(PaddingConfig1d::Explicit(2, 2));
        for _ in 0..3 {
            prenet.push(padded.clone().init(device));
        }
        let prenet_proj = Conv1dConfig::new(config.hidden, config.hidden, 1).init(device);

        let mut prenet_norms = Vec::with_capacity(3);
        for _ in 0..3 {
            prenet_norms.push(AffineNorm::new(config.hidden, device));
        }

        let mut attentions = Vec::with_capacity(config.layers);
        let mut feed_forwards = Vec::with_capacity(config.layers);
        let mut block_norms = Vec::with_capacity(config.layers);
        for _ in 0..config.layers {
            attentions.push(Attention::new(config.hidden, device));
            feed_forwards.push(FeedForward::new(config.hidden, config.ffn_hidden, device));
            block_norms.push((
                AffineNorm::new(config.hidden, device),
                AffineNorm::new(config.hidden, device),
            ));
        }

        Self {
            embedding,
            prenet,
            prenet_proj,
            prenet_norms,
            block_norms,
            attentions,
            feed_forwards,
            proj_m: Conv1dConfig::new(config.hidden, config.n_mels, 1).init(device),
            duration_conv1: Conv1dConfig::new(config.hidden, config.duration_hidden, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            duration_conv2: Conv1dConfig::new(config.duration_hidden, config.duration_hidden, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            duration_proj: Conv1dConfig::new(config.duration_hidden, 1, 1).init(device),
        }
    }

    /// 前向：返回 mel 先验与对数时长。
    pub fn forward(&self, tokens: Tensor<2, Int>) -> (Tensor<3>, Tensor<2>) {
        // 词嵌入输出 [b, seq, hidden]，转成 [b, hidden, seq]。
        let mut hidden = self.embedding.forward(tokens).transpose();
        let [batch, _hidden_dim, frames] = hidden.dims();

        for index in 0..self.prenet.len() {
            let conv = relu(self.prenet[index].forward(hidden.clone()));
            hidden = self.prenet_norms[index].forward(conv);
        }
        hidden = self.prenet_proj.forward(hidden);

        for index in 0..self.attentions.len() {
            let (norm1, norm2) = &self.block_norms[index];
            let attn_out = self.attentions[index].forward(norm1.forward(hidden.clone()));
            hidden = hidden + attn_out;
            let ffn_out = self.feed_forwards[index].forward(norm2.forward(hidden.clone()));
            hidden = hidden + ffn_out;
        }

        let mel = self.proj_m.forward(hidden.clone());
        let durations = relu(self.duration_conv1.forward(hidden));
        let durations = relu(self.duration_conv2.forward(durations));
        let proj_dur = self.duration_proj.forward(durations);
        let durations = proj_dur.reshape([batch, frames]);
        (mel, durations)
    }
}
// 归一化参数名顺序，与基座图内导出顺序一致（weight 与 bias 交替）。
fn normalization_names(layers: usize) -> Vec<String> {
    let mut names = Vec::new();
    for index in 0..3 {
        for part in ["weight", "bias"] {
            names.push(format!("model.encoder.prenet.norm{index}.{part}"));
        }
    }
    for index in 0..layers {
        for slot in ["1", "2"] {
            for part in ["weight", "bias"] {
                names.push(format!("model.encoder.encoder.norm{index}_{slot}.{part}"));
            }
        }
    }
    names
}

fn fill3(param: &mut Param<Tensor<3>>, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<3>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    *param = Param::from_tensor(t);
}

fn fill2(param: &mut Param<Tensor<2>>, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<2>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    *param = Param::from_tensor(t);
}

fn fill1(param: &mut Param<Tensor<1>>, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    *param = Param::from_tensor(t);
}

/// 按名字把一个命名 ONNX 权重张量填入编码器对应参数。
///
/// 只处理 `model.encoder.*.weight`；未知名字静默忽略，便于复用基座权重。
/// 按名字把一个命名 ONNX 权重张量填入编码器对应参数。
///
/// 识别 `model.encoder.*.{weight,bias}`；未知名字静默忽略，便于复用基座权重。
pub fn apply_named_weight(encoder: &mut BurnBaseEncoder, tensor: &OnnxTensor, device: &Device) {
    let name = &tensor.name;
    let suffix: &str = name.rsplit('.').next().unwrap_or("");
    let container = name
        .strip_suffix(".weight")
        .or_else(|| name.strip_suffix(".bias"))
        .unwrap_or(name);

    // 词嵌入只有 weight。
    if name == "model.encoder.emb.weight" {
        fill2(&mut encoder.embedding.weight, &tensor.data, &tensor.dims, device);
        return;
    }
    // 归一化参数走 apply_normalizations，这里跳过命名样子的 norm weight/bias。
    if container.contains("norm") && container.contains("prenet") && container.contains("encoder") {
        // 不匹配具体归一化见 apply_normalizations
    }

    // prenet 卷积层。
    if let Some(idxstr) = container
        .strip_prefix("model.encoder.prenet.conv_layers.")
        .and_then(|r| r.split('.').next())
    {
        let index: usize = idxstr.parse().unwrap_or(usize::MAX);
        if index < encoder.prenet.len() {
            let w = &mut encoder.prenet[index];
            match suffix {
                "weight" => fill3(&mut w.weight, &tensor.data, &tensor.dims, device),
                "bias" => fill1(&mut w.bias.as_mut().unwrap(), &tensor.data, &tensor.dims, device),
                _ => {}
            }
        }
        return;
    }
    if container == "model.encoder.prenet.proj" {
        match suffix {
            "weight" => fill3(&mut encoder.prenet_proj.weight, &tensor.data, &tensor.dims, device),
            "bias" => fill1(&mut encoder.prenet_proj.bias.as_mut().unwrap(), &tensor.data, &tensor.dims, device),
            _ => {}
        }
        return;
    }
    if let Some(rest) = container.strip_prefix("model.encoder.encoder.attn_layers.") {
        let (idx, after) = rest.split_once('.').unwrap_or((rest, ""));
        let index: usize = idx.parse().unwrap_or(usize::MAX);
        if index >= encoder.attentions.len() {
            return;
        }
        let attn = &mut encoder.attentions[index];
        let conv = match after {
            "conv_q" => &mut attn.query,
            "conv_k" => &mut attn.key,
            "conv_v" => &mut attn.value,
            "conv_o" => &mut attn.output,
            _ => return,
        };
        match suffix {
            "weight" => fill3(&mut conv.weight, &tensor.data, &tensor.dims, device),
            "bias" => fill1(&mut conv.bias.as_mut().unwrap(), &tensor.data, &tensor.dims, device),
            _ => {}
        }
        return;
    }
    if let Some(rest) = container.strip_prefix("model.encoder.encoder.ffn_layers.") {
        let (idx, after) = rest.split_once('.').unwrap_or((rest, ""));
        let index: usize = idx.parse().unwrap_or(usize::MAX);
        if index >= encoder.feed_forwards.len() {
            return;
        }
        let ff = &mut encoder.feed_forwards[index];
        let conv = match after {
            "conv_1" => &mut ff.conv1,
            "conv_2" => &mut ff.conv2,
            _ => return,
        };
        match suffix {
            "weight" => fill3(&mut conv.weight, &tensor.data, &tensor.dims, device),
            "bias" => fill1(&mut conv.bias.as_mut().unwrap(), &tensor.data, &tensor.dims, device),
            _ => {}
        }
        return;
    }
    if container == "model.encoder.proj_m" {
        match suffix {
            "weight" => fill3(&mut encoder.proj_m.weight, &tensor.data, &tensor.dims, device),
            "bias" => fill1(&mut encoder.proj_m.bias.as_mut().unwrap(), &tensor.data, &tensor.dims, device),
            _ => {}
        }
        return;
    }
    if let Some(rest) = container.strip_prefix("model.encoder.proj_w.") {
        let (proj, _) = rest.split_once('.').unwrap_or((rest, ""));
        let conv = match proj {
            "conv_1" => &mut encoder.duration_conv1,
            "conv_2" => &mut encoder.duration_conv2,
            "proj" => &mut encoder.duration_proj,
            _ => return,
        };
        match suffix {
            "weight" => fill3(&mut conv.weight, &tensor.data, &tensor.dims, device),
            "bias" => fill1(&mut conv.bias.as_mut().unwrap(), &tensor.data, &tensor.dims, device),
            _ => {}
        }
    }
}

/// 把匿名归一化常量按图内顺序填入编码器各 AffineNorm。
pub fn apply_normalizations(
    encoder: &mut BurnBaseEncoder,
    tensors: &[OnnxTensor],
    hidden: usize,
    layers: usize,
    device: &Device,
) -> Result<(), String> {
    let mut anonymous: Vec<&OnnxTensor> = tensors
        .iter()
        .filter(|tensor| tensor.name.starts_with("onnx::") && tensor.dims == vec![1, hidden, 1])
        .collect();
    anonymous.sort_by_key(|tensor| {
        tensor
            .name
            .rsplit('_')
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let names = normalization_names(layers);
    if anonymous.len() != names.len() {
        return Err(format!(
            "归一化参数数量不符：图里 {} 个，模型需要 {} 个",
            anonymous.len(),
            names.len()
        ));
    }

    let shape = vec![1, hidden, 1];
    // 用索引数组避免同一字段的多重可变借用。
    let mut targets: Vec<(bool, usize, usize)> = Vec::with_capacity(names.len()); // (is_prenet, idx, slot)
    for i in 0..3 {
        targets.push((true, i, 0));
        targets.push((true, i, 1));
    }
    for i in 0..layers {
        targets.push((false, i, 0));
        targets.push((false, i, 1));
        targets.push((false, i, 2));
        targets.push((false, i, 3));
    }

    for (tensor, &(is_prenet, idx, slot)) in anonymous.iter().zip(targets.iter()) {
        let t = Tensor::<3>::from_data(TensorData::new(tensor.data.clone(), shape.clone()), device);
        let t = Param::from_tensor(t);
        if is_prenet {
            if slot == 0 {
                encoder.prenet_norms[idx].weight = t;
            } else {
                encoder.prenet_norms[idx].bias = t;
            }
        } else {
            match slot {
                0 => encoder.block_norms[idx].0.weight = t,
                1 => encoder.block_norms[idx].0.bias = t,
                2 => encoder.block_norms[idx].1.weight = t,
                _ => encoder.block_norms[idx].1.bias = t,
            }
        }
    }
    Ok(())
}

/// 从基座 ONNX 张量按名构造编码器（含命名权重与匿名归一化）。
pub fn from_onnx_tensors(
    config: &BurnBaseEncoderConfig,
    tensors: &[OnnxTensor],
    device: &Device,
) -> Result<BurnBaseEncoder, String> {
    let mut encoder = BurnBaseEncoder::init(config, device);
    for tensor in tensors {
        if tensor.name.ends_with(".weight") && tensor.name.starts_with("model.encoder.") {
            apply_named_weight(&mut encoder, tensor, device);
        }
    }
    apply_normalizations(&mut encoder, tensors, config.hidden, config.layers, device)?;
    Ok(encoder)
}


#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::burn_backend::train_device;
    use super::*;

    /// 在当前平台设备上跑一次前向，验证形状与链路可用。
    #[test]
    fn forward_produces_mel_and_durations() {
        let device = train_device();
        let config = BurnBaseEncoderConfig::default();
        let encoder = BurnBaseEncoder::init(&config, &device);

        let tokens = Tensor::<2, Int>::from_data(
            TensorData::new(vec![1u32, 2, 3, 4, 5], vec![1, 5]),
            &device,
        );
        let (mel, durations) = encoder.forward(tokens);
        assert_eq!(mel.shape().dims(), [1, 80, 5]);
        assert_eq!(durations.shape().dims(), [1, 5]);
    }

    /// 从基座 ONNX 加载权重到 burn 编码器，验证权重桥可用且前向形状正确。
    #[test]
    fn loads_from_real_onnx_and_forwards() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx");
        if !path.exists() {
            return;
        }
        let tensors = super::super::onnx_weights::read_initializers(&path).unwrap();
        let device = train_device();
        let config = BurnBaseEncoderConfig::default();
        let encoder = from_onnx_tensors(&config, &tensors, &device).unwrap();

        let tokens = Tensor::<2, Int>::from_data(
            TensorData::new(vec![1u32, 2, 3, 4, 5], vec![1, 5]),
            &device,
        );
        let (mel, durations) = encoder.forward(tokens);
        assert_eq!(mel.shape().dims(), [1, 80, 5]);
        assert_eq!(durations.shape().dims(), [1, 5]);
    }
}
