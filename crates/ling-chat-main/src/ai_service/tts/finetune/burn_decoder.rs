//! 用 burn 实现的基座解码器，与 candle 版逐层对齐。
//!
//! 网络结构：时间 MLP、下行路径（时间条件卷积残差块 + 自注意力 + 卷积下采样）、
//! 中部（两个残差注意力块）、上行路径（残差注意力块 + 上采样/转置卷积）、
//! 收尾卷积与 mel 投影。转置卷积用 burn 原生实现，省去 candle 为绕开自动求导
//! 限制而写的手工等价版本。

use burn::module::{Module, Param};
use burn::nn::PaddingConfig1d;
use burn::nn::conv::{Conv1d, Conv1dConfig, ConvTranspose1d, ConvTranspose1dConfig};
use burn::prelude::*;
use burn::tensor::activation::{mish, silu};

use super::conv_lora::{ConvLora, ConvTransposeLora};

/// 解码器超参数（burn 版，字段与 candle 版对应）。
#[derive(Debug, Clone)]
pub struct BurnBaseDecoderConfig {
    /// 隐层通道数。
    pub channels: usize,
    /// 时间嵌入维度。
    pub time_dim: usize,
    /// 时间 MLP 隐层。
    pub time_hidden: usize,
    /// mel 通道数。
    pub n_mels: usize,
    /// 解码器输入通道数：加噪 mel 与先验 mel 拼接。
    pub input_channels: usize,
    /// 卷积 LoRA 秩；0 表示不挂载。
    ///
    /// 覆盖残差块的 conv1/conv2/res_conv、下行采样卷积、上行采样卷积、
    /// 收尾卷积与 mel 投影。下投影是 1x1 卷积，上投影保持原卷积核/步长，
    /// 导出时能精确折叠回基座卷积权重（转置卷积走单独的低秩形式）。
    pub conv_lora_rank: usize,
}

impl Default for BurnBaseDecoderConfig {
    fn default() -> Self {
        Self {
            channels: 256,
            time_dim: 160,
            time_hidden: 1024,
            n_mels: 80,
            input_channels: 160,
            conv_lora_rank: 0,
        }
    }
}

/// 逐通道风格调制（FiLM），初始为恒等映射。
///
/// 训练时对归一化输出做 `y * gamma + beta`。导出前把 gamma/beta 折进归一化
/// 自带的 weight/bias，因此导出的 ONNX 图结构不变，推理端零改动。
#[derive(Module, Debug)]
struct StyleFilm {
    gamma: Param<Tensor<1>>,
    beta: Param<Tensor<1>>,
}

impl StyleFilm {
    fn new(channels: usize, device: &Device) -> Self {
        Self {
            gamma: Param::from_tensor(Tensor::<1>::ones([channels], device)),
            beta: Param::from_tensor(Tensor::<1>::zeros([channels], device)),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, channels, frames] = xs.dims();
        let gamma = self
            .gamma
            .val()
            .reshape([1, channels, 1])
            .expand([batch, channels, frames]);
        let beta = self
            .beta
            .val()
            .reshape([1, channels, 1])
            .expand([batch, channels, frames]);
        xs * gamma + beta
    }

    /// 折叠进二维仿射参数（形状 `[channels, 1]`）。
    fn fold_2d(&self, weight: Tensor<2>, bias: Tensor<2>) -> (Tensor<2>, Tensor<2>) {
        let [channels, _] = weight.dims();
        let gamma = self.gamma.val().reshape([channels, 1]);
        let beta = self.beta.val().reshape([channels, 1]);
        (weight * gamma.clone(), bias * gamma + beta)
    }

    /// 折叠进一维仿射参数（形状 `[channels]`）。
    fn fold_1d(&self, weight: Tensor<1>, bias: Tensor<1>) -> (Tensor<1>, Tensor<1>) {
        let gamma = self.gamma.val();
        let beta = self.beta.val();
        (weight * gamma.clone(), bias * gamma + beta)
    }

    /// 折叠后把自己恢复成恒等映射，避免同一模块被重复调制。
    fn reset(&mut self) {
        self.gamma = Param::from_tensor(self.gamma.val().ones_like());
        self.beta = Param::from_tensor(self.beta.val().zeros_like());
    }
}

/// 分组归一化加逐通道仿射，等价 candle 的 GroupNormAffine。
///
/// 把通道分 8 组，组内按（通道 x 帧）做纯归一化，再施加逐通道仿射
/// （weight/bias 形状 [channels, 1]，由匿名 onnx 常量按映射表落位）。
#[derive(Module, Debug)]
struct GroupNormAffine {
    /// 纯分组归一化（无内部仿射），仿射由 weight/bias 手工施加。
    norm: burn::nn::GroupNorm,
    weight: Param<Tensor<2>>,
    bias: Param<Tensor<2>>,
    /// 风格适配器；初始恒等，导出时折叠进 weight/bias。
    style_film: StyleFilm,
}

impl GroupNormAffine {
    fn new(channels: usize, groups: usize, device: &Device) -> Self {
        use burn::nn::GroupNormConfig;
        Self {
            norm: GroupNormConfig::new(groups, channels)
                .with_affine(false)
                .with_epsilon(1e-5)
                .init(device),
            weight: Param::from_tensor(Tensor::<2>::ones([channels, 1], device)),
            bias: Param::from_tensor(Tensor::<2>::zeros([channels, 1], device)),
            style_film: StyleFilm::new(channels, device),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, channels, frames] = xs.dims();
        let normed = self.norm.forward(xs);
        let weight = self.weight.val().unsqueeze_dim::<3>(0); // [1, channels, 1]
        let bias = self.bias.val().unsqueeze_dim::<3>(0);
        let out = normed * weight.expand([batch, channels, frames])
            + bias.expand([batch, channels, frames]);
        self.style_film.forward(out)
    }

    /// 把风格 FiLM 折进本层的仿射参数。
    fn fold_style_film(&mut self) {
        let weight = self.weight.val();
        let bias = self.bias.val();
        let (weight, bias) = self.style_film.fold_2d(weight, bias);
        self.weight = Param::from_tensor(weight);
        self.bias = Param::from_tensor(bias);
        self.style_film.reset();
    }
}

/// 时间嵌入与两层投影（内部 SiLU）。
#[derive(Module, Debug)]
struct TimeMlp {
    linear1: burn::nn::Linear,
    linear2: burn::nn::Linear,
}

impl TimeMlp {
    fn new(config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        use burn::nn::LinearConfig;
        Self {
            linear1: LinearConfig::new(config.time_dim, config.time_hidden).init(device),
            linear2: LinearConfig::new(config.time_hidden, config.time_hidden).init(device),
        }
    }

    /// 输入 [batch, time_dim]，输出 [batch, time_hidden]。
    fn forward(&self, time: Tensor<2>) -> Tensor<2> {
        let hidden = self.linear1.forward(time);
        self.linear2.forward(silu(hidden))
    }
}

/// 时间条件卷积残差块。
#[derive(Module, Debug)]
struct ResBlock {
    /// 块级时间条件投影：time_hidden -> channels（图上 mlp.1 Gemm）。
    time_proj: burn::nn::Linear,
    conv1: Conv1d,
    conv2: Conv1d,
    res_conv: Conv1d,
    /// 卷积低秩旁路；`None` 表示未启用，导出时折进对应基座卷积。
    conv_lora1: Option<ConvLora>,
    conv_lora2: Option<ConvLora>,
    conv_lora_res: Option<ConvLora>,
    norm1: GroupNormAffine,
    norm2: GroupNormAffine,
    channels: usize,
}

impl ResBlock {
    fn new(input_channels: usize, config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        use burn::nn::LinearConfig;
        let channels = config.channels;
        let rank = config.conv_lora_rank;
        let conv_lora1 =
            (rank > 0).then(|| ConvLora::new(input_channels, channels, 3, 1, 1, rank, device));
        let conv_lora2 =
            (rank > 0).then(|| ConvLora::new(channels, channels, 3, 1, 1, rank, device));
        let conv_lora_res =
            (rank > 0).then(|| ConvLora::new(input_channels, channels, 1, 1, 0, rank, device));
        Self {
            time_proj: LinearConfig::new(config.time_hidden, channels).init(device),
            conv1: Conv1dConfig::new(input_channels, channels, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            conv2: Conv1dConfig::new(channels, channels, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            res_conv: Conv1dConfig::new(input_channels, channels, 1).init(device),
            conv_lora1,
            conv_lora2,
            conv_lora_res,
            norm1: GroupNormAffine::new(channels, 8, device),
            norm2: GroupNormAffine::new(channels, 8, device),
            channels,
        }
    }

    /// 输入 [batch, input_channels, frames]，输出 [batch, channels, frames]。
    fn forward(&self, xs: Tensor<3>, time: Tensor<2>) -> Tensor<3> {
        let [batch, _ic, _frames] = xs.dims();
        let mut res = self.res_conv.forward(xs.clone());
        if let Some(lora) = &self.conv_lora_res {
            res = res + lora.forward(xs.clone());
        }
        // 块级时间条件：mish(time) -> 线性 -> [batch, channels, 1] 广播加。
        let bias = self
            .time_proj
            .forward(mish(time))
            .reshape([batch, self.channels, 1]);
        let mut conv1 = self.conv1.forward(xs.clone());
        if let Some(lora) = &self.conv_lora1 {
            conv1 = conv1 + lora.forward(xs);
        }
        let hidden = self.norm1.forward(conv1);
        let hidden = mish(hidden) + bias;
        let mut conv2 = self.conv2.forward(hidden.clone());
        if let Some(lora) = &self.conv_lora2 {
            conv2 = conv2 + lora.forward(hidden);
        }
        let hidden = self.norm2.forward(conv2);
        let hidden = mish(hidden);
        res + hidden
    }

    /// 把两个归一化上的风格 FiLM 折进仿射参数。
    fn fold_style_film(&mut self) {
        self.norm1.fold_style_film();
        self.norm2.fold_style_film();
    }

    /// 把卷积 LoRA 折进基座卷积，并移除旁路。
    fn fold_conv_lora(&mut self) {
        if let Some(lora) = self.conv_lora1.take() {
            lora.fold_into(&mut self.conv1);
        }
        if let Some(lora) = self.conv_lora2.take() {
            lora.fold_into(&mut self.conv2);
        }
        if let Some(lora) = self.conv_lora_res.take() {
            lora.fold_into(&mut self.res_conv);
        }
    }
}

/// 带仿射参数的归一化，沿通道维（注意力块的 norm1/norm3）。
#[derive(Module, Debug)]
struct Affine {
    weight: Param<Tensor<1>>,
    bias: Param<Tensor<1>>,
    /// 风格适配器；初始恒等，导出时折叠进 weight/bias。
    style_film: StyleFilm,
}

impl Affine {
    fn new(channels: usize, device: &Device) -> Self {
        Self {
            weight: Param::from_tensor(Tensor::<1>::ones([channels], device)),
            bias: Param::from_tensor(Tensor::<1>::zeros([channels], device)),
            style_film: StyleFilm::new(channels, device),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, channels, frames] = xs.dims();
        let mean = xs.clone().mean_dims(&[1]);
        let centered = xs.clone() - mean;
        let variance = centered.clone().powf_scalar(2.0).mean_dims(&[1]);
        let normed = centered / (variance + 1e-5).sqrt();
        let weight = self
            .weight
            .val()
            .reshape([1, channels, 1])
            .expand([batch, channels, frames]);
        let bias = self
            .bias
            .val()
            .reshape([1, channels, 1])
            .expand([batch, channels, frames]);
        self.style_film.forward(normed * weight + bias)
    }

    /// 把风格 FiLM 折进本层的仿射参数。
    fn fold_style_film(&mut self) {
        let weight = self.weight.val();
        let bias = self.bias.val();
        let (weight, bias) = self.style_film.fold_1d(weight, bias);
        self.weight = Param::from_tensor(weight);
        self.bias = Param::from_tensor(bias);
        self.style_film.reset();
    }
}

/// 自注意力加前馈块（多头注意力，2 头）。
#[derive(Module, Debug)]
struct AttnBlock {
    norm1: Affine,
    norm3: Affine,
    q: burn::nn::Linear,
    k: burn::nn::Linear,
    v: burn::nn::Linear,
    to_out: burn::nn::Linear,
    ff_proj: burn::nn::Linear,
    ff_out: burn::nn::Linear,
    ff_alpha: Param<Tensor<1>>,
    ff_beta: Param<Tensor<1>>,
}

impl AttnBlock {
    fn new(config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        use burn::nn::LinearConfig;
        let channels = config.channels;
        let inner = channels / 2;
        Self {
            norm1: Affine::new(channels, device),
            norm3: Affine::new(channels, device),
            q: LinearConfig::new(channels, inner)
                .with_bias(false)
                .init(device),
            k: LinearConfig::new(channels, inner)
                .with_bias(false)
                .init(device),
            v: LinearConfig::new(channels, inner)
                .with_bias(false)
                .init(device),
            to_out: LinearConfig::new(inner, channels).init(device),
            ff_proj: LinearConfig::new(channels, config.time_hidden).init(device),
            ff_out: LinearConfig::new(config.time_hidden, channels).init(device),
            ff_alpha: Param::from_tensor(Tensor::<1>::ones([config.time_hidden], device)),
            ff_beta: Param::from_tensor(Tensor::<1>::zeros([config.time_hidden], device)),
        }
    }

    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        let [batch, channels, frames] = xs.dims();
        let heads = 2;
        let inner = channels / 2;
        let head_dim = inner / heads;

        let normed = self.norm1.forward(xs.clone()).transpose(); // [B, F, C]
        let q = self
            .q
            .forward(normed.clone())
            .reshape([batch, frames, heads, head_dim]);
        let q = q.permute([0, 2, 1, 3]); // [B, heads, F, head_dim]
        let k = self
            .k
            .forward(normed.clone())
            .reshape([batch, frames, heads, head_dim]);
        let k = k.permute([0, 2, 1, 3]);
        let v = self
            .v
            .forward(normed)
            .reshape([batch, frames, heads, head_dim]);
        let v = v.permute([0, 2, 1, 3]);

        let scale = 1.0 / (head_dim as f64).sqrt();
        let scores = q.matmul(k.transpose()) * scale;
        let weights = burn::tensor::activation::softmax(scores, 3);
        let context = weights
            .matmul(v)
            .transpose()
            .reshape([batch, frames, inner]); // [B,F,inner]
        let attended = self.to_out.forward(context).transpose(); // [B,C,F]
        let hidden = attended + xs;

        let normed3 = self.norm3.forward(hidden.clone()).transpose(); // [B,F,C]
        let [b, f, c] = normed3.clone().dims();
        let projected = self.ff_proj.forward(normed3.reshape([b * f, c]));
        let activated = gelu_parametric(projected, self.ff_alpha.val(), self.ff_beta.val());
        let feed = self
            .ff_out
            .forward(activated)
            .reshape([b, f, c])
            .transpose(); // [B,C,F]
        hidden + feed
    }

    /// 把两个归一化上的风格 FiLM 折进仿射参数。
    fn fold_style_film(&mut self) {
        self.norm1.fold_style_film();
        self.norm3.fold_style_film();
    }
}

/// 参数化 GELU：u + sin(u*exp(alpha))^2 / (exp(beta) + 1e-9)。
/// 参数化 GELU：u + sin(u*exp(alpha))^2 / (exp(beta) + 1e-9)。
fn gelu_parametric(x: Tensor<2>, alpha: Tensor<1>, beta: Tensor<1>) -> Tensor<2> {
    let [_, cols] = x.dims();
    // 展开成 [1, cols] 与帧维广播相乘。
    let alpha = alpha.reshape([1, cols]);
    let beta = beta.reshape([1, cols]);
    let scaled = x.clone() * alpha.exp();
    let sin_sq = scaled.sin().powf_scalar(2.0);
    let denom = beta.exp().add_scalar(1e-9);
    x + sin_sq / denom
}

/// 上采样算子：up_blocks.{i}.2，i=0 转置卷积，i=1 普通卷积。
#[derive(Module, Debug)]
enum UpSampler {
    Transpose(ConvTranspose1d),
    Conv(Conv1d),
}

impl UpSampler {
    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        match self {
            UpSampler::Transpose(c) => c.forward(xs),
            UpSampler::Conv(c) => c.forward(xs),
        }
    }
}

/// 上采样算子的低秩旁路：转置卷积与普通卷积形式不同，导出时分别折叠。
#[derive(Module, Debug)]
enum SamplerLora {
    Transpose(ConvTransposeLora),
    Conv(ConvLora),
}

impl SamplerLora {
    fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        match self {
            SamplerLora::Transpose(lora) => lora.forward(xs),
            SamplerLora::Conv(lora) => lora.forward(xs),
        }
    }

    fn fold_into(&self, sampler: &mut UpSampler) {
        match (self, sampler) {
            (SamplerLora::Transpose(lora), UpSampler::Transpose(conv)) => lora.fold_into(conv),
            (SamplerLora::Conv(lora), UpSampler::Conv(conv)) => lora.fold_into(conv),
            _ => {
                debug_assert!(false, "上采样低秩旁路与基座算子类型不一致");
            },
        }
    }
}

/// 解码器：时间 MLP、下行、中部、上行与收尾投影。
#[derive(Module, Debug)]
pub struct BurnBaseDecoder {
    time_mlp: TimeMlp,
    down: Vec<(ResBlock, AttnBlock, Conv1d)>,
    mid: Vec<(ResBlock, AttnBlock)>,
    up: Vec<(ResBlock, AttnBlock, UpSampler)>,
    final_norm: GroupNormAffine,
    final_block: Conv1d,
    final_proj: Conv1d,
    /// 下行采样卷积的旁路；与元组字段分开存，保证旧检查点的键名不变。
    down_conv_lora: Vec<Option<ConvLora>>,
    /// 上行采样算子的旁路（转置卷积/普通卷积两种形式）。
    up_conv_lora: Vec<Option<SamplerLora>>,
    final_block_conv_lora: Option<ConvLora>,
    final_proj_conv_lora: Option<ConvLora>,
}

impl BurnBaseDecoder {
    /// 用随机权重初始化解码器。
    pub fn init(config: &BurnBaseDecoderConfig, device: &Device) -> Self {
        let rank = config.conv_lora_rank;
        let mut down = Vec::with_capacity(2);
        let mut down_conv_lora = Vec::with_capacity(2);
        for index in 0..2 {
            let input_channels = if index == 0 {
                config.input_channels
            } else {
                config.channels
            };
            let stride = if index == 0 { 2 } else { 1 };
            down.push((
                ResBlock::new(input_channels, config, device),
                AttnBlock::new(config, device),
                Conv1dConfig::new(config.channels, config.channels, 3)
                    .with_stride(stride)
                    .with_padding(PaddingConfig1d::Explicit(1, 1))
                    .init(device),
            ));
            down_conv_lora.push((rank > 0).then(|| {
                ConvLora::new(config.channels, config.channels, 3, stride, 1, rank, device)
            }));
        }

        let mid = (0..2)
            .map(|_| {
                (
                    ResBlock::new(config.channels, config, device),
                    AttnBlock::new(config, device),
                )
            })
            .collect();

        let mut up = Vec::with_capacity(2);
        let mut up_conv_lora = Vec::with_capacity(2);
        for index in 0..2 {
            let (sampler, lora) = if index == 0 {
                (
                    UpSampler::Transpose(
                        ConvTranspose1dConfig::new([config.channels, config.channels], 4)
                            .with_stride(2)
                            .with_padding(1)
                            .init(device),
                    ),
                    (rank > 0).then(|| {
                        SamplerLora::Transpose(ConvTransposeLora::new(
                            config.channels,
                            config.channels,
                            4,
                            2,
                            1,
                            rank,
                            device,
                        ))
                    }),
                )
            } else {
                (
                    UpSampler::Conv(
                        Conv1dConfig::new(config.channels, config.channels, 3)
                            .with_padding(PaddingConfig1d::Explicit(1, 1))
                            .init(device),
                    ),
                    (rank > 0).then(|| {
                        SamplerLora::Conv(ConvLora::new(
                            config.channels,
                            config.channels,
                            3,
                            1,
                            1,
                            rank,
                            device,
                        ))
                    }),
                )
            };
            up.push((
                ResBlock::new(config.channels * 2, config, device),
                AttnBlock::new(config, device),
                sampler,
            ));
            up_conv_lora.push(lora);
        }

        Self {
            time_mlp: TimeMlp::new(config, device),
            down,
            mid,
            up,
            final_norm: GroupNormAffine::new(config.channels, 8, device),
            final_block: Conv1dConfig::new(config.channels, config.channels, 3)
                .with_padding(PaddingConfig1d::Explicit(1, 1))
                .init(device),
            final_proj: Conv1dConfig::new(config.channels, config.n_mels, 1).init(device),
            down_conv_lora,
            up_conv_lora,
            final_block_conv_lora: (rank > 0)
                .then(|| ConvLora::new(config.channels, config.channels, 3, 1, 1, rank, device)),
            final_proj_conv_lora: (rank > 0)
                .then(|| ConvLora::new(config.channels, config.n_mels, 1, 1, 0, rank, device)),
        }
    }

    /// 返回一个把风格 FiLM 与卷积 LoRA 折叠进基座权重的副本，供 ONNX 导出使用。
    ///
    /// FiLM 折叠是精确等价变换：`(x*w+b)*gamma+beta == x*(w*gamma)+(b*gamma+beta)`；
    /// 卷积 LoRA 则把 `B @ A` 低秩增量直接加到基座卷积核。导出桥仍然只认识基座
    /// 原有的 weight/bias，因此不需要给 ONNX 图加节点。
    pub fn folded_for_export(&self) -> Self {
        let mut decoder = self.clone();
        for (block, attention, _) in &mut decoder.down {
            block.fold_style_film();
            block.fold_conv_lora();
            attention.fold_style_film();
        }
        for (index, (_, _, conv)) in decoder.down.iter_mut().enumerate() {
            if let Some(lora) = decoder.down_conv_lora.get_mut(index).and_then(Option::take) {
                lora.fold_into(conv);
            }
        }
        for (block, attention) in &mut decoder.mid {
            block.fold_style_film();
            block.fold_conv_lora();
            attention.fold_style_film();
        }
        for (index, (block, attention, sampler)) in decoder.up.iter_mut().enumerate() {
            block.fold_style_film();
            block.fold_conv_lora();
            attention.fold_style_film();
            if let Some(lora) = decoder.up_conv_lora.get_mut(index).and_then(Option::take) {
                lora.fold_into(sampler);
            }
        }
        decoder.final_norm.fold_style_film();
        if let Some(lora) = decoder.final_block_conv_lora.take() {
            lora.fold_into(&mut decoder.final_block);
        }
        if let Some(lora) = decoder.final_proj_conv_lora.take() {
            lora.fold_into(&mut decoder.final_proj);
        }
        decoder
    }

    /// 前向：输入加噪 mel、文本隐状态与时间，输出速度场。
    pub fn forward(&self, noisy: Tensor<3>, text: Tensor<3>, time: Tensor<2>) -> Tensor<3> {
        let time = self.time_mlp.forward(time);
        let mut hidden = Tensor::cat(vec![noisy, text], 1);

        let mut skips: Vec<Tensor<3>> = Vec::with_capacity(2);
        for (index, (block, attention, down)) in self.down.iter().enumerate() {
            hidden = block.forward(hidden, time.clone());
            hidden = attention.forward(hidden);
            skips.push(hidden.clone());
            let sampled = down.forward(hidden);
            hidden = match self.down_conv_lora.get(index) {
                Some(Some(lora)) => {
                    // 旁路必须吃采样前的同一个输入，不能复用采样后的 hidden。
                    let skip = skips.last().expect("skip 刚写入").clone();
                    sampled + lora.forward(skip)
                },
                _ => sampled,
            };
        }
        for (block, attention) in &self.mid {
            hidden = block.forward(hidden, time.clone());
            hidden = attention.forward(hidden);
        }
        for (index, (block, attention, sampler)) in self.up.iter().enumerate() {
            let skip = skips.pop().expect("上行路径的 skip 数量与下行路径对齐");
            let [_, _, skip_frames] = skip.dims();
            let [_, _, f] = hidden.dims();
            if f != skip_frames {
                hidden = hidden.narrow(2, 0, skip_frames);
            }
            hidden = Tensor::cat(vec![hidden, skip], 1);
            hidden = block.forward(hidden, time.clone());
            hidden = attention.forward(hidden);
            let before = hidden.clone();
            let sampled = sampler.forward(hidden);
            hidden = match self.up_conv_lora.get(index) {
                Some(Some(lora)) => sampled + lora.forward(before),
                _ => sampled,
            };
        }
        let before_final = hidden.clone();
        let mut hidden = self.final_block.forward(hidden);
        if let Some(lora) = &self.final_block_conv_lora {
            hidden = hidden + lora.forward(before_final);
        }
        let hidden = self.final_norm.forward(hidden);
        let hidden = mish(hidden);
        let projected = self.final_proj.forward(hidden.clone());
        match &self.final_proj_conv_lora {
            Some(lora) => projected + lora.forward(hidden),
            None => projected,
        }
    }
}

// ---- 权重桥：从 ONNX 基座（命名张量 + decoder_weight_map.json 匿名常量）填充解码器 ----

/// 把一组 float 数据装入一个线性层权重。`src` 以 `onnx::MatMul_` 开头时代表 ONNX
/// MatMul 的 [in, out] 布局，与 burn Linear 的 `weight: [d_input, d_output]` 一致，
/// 无需转置；其它来源（Gemm/命名）是 `[out, in]`，需转置再填。
fn set_linear_weight(
    linear: &mut burn::nn::Linear,
    data: &[f32],
    dims: &[usize],
    transposed: bool,
    device: &Device,
) {
    if dims.len() != 2 {
        panic!("set_linear_weight got non-2 dims: {:?}", dims);
    }
    let t = if transposed {
        Tensor::<2>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device)
            .permute([1, 0])
    } else {
        Tensor::<2>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device)
    };
    linear.weight = Param::from_tensor(t);
}

fn set_linear_bias(linear: &mut burn::nn::Linear, data: &[f32], dims: &[usize], device: &Device) {
    if let Some(bias) = linear.bias.as_mut() {
        let t = Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
        *bias = Param::from_tensor(t);
    }
}

/// 给卷积权重填充数据。ONNX 普通/转置卷积权重布局均为 `[c_out, c_in, k]`，而 burn
/// 的 `Conv1d`/`ConvTranspose1d` 权重为 `[c_in, c_out, k]`，需交换前两维。
/// 卷积权重（普通与转置卷积）布局同为 `[c_out, c_in, k]`，与 burn 的
/// `Conv1d::weight`、`ConvTranspose1d::weight` 一致，无需转置。
fn set_conv_weight(conv: &mut Conv1d, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<3>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    conv.weight = Param::from_tensor(t);
}

fn set_conv_bias(conv: &mut Conv1d, data: &[f32], dims: &[usize], device: &Device) {
    if let Some(bias) = conv.bias.as_mut() {
        let t = Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
        *bias = Param::from_tensor(t);
    }
}

#[allow(dead_code)]
fn set_groups_weight(group: &mut GroupNormAffine, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<2>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    group.weight = Param::from_tensor(t);
}

#[allow(dead_code)]
fn set_groups_bias(group: &mut GroupNormAffine, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<2>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    group.bias = Param::from_tensor(t);
}

fn set_affine(affine: &mut Affine, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    affine.weight = Param::from_tensor(t);
}

fn set_affine_bias(affine: &mut Affine, data: &[f32], dims: &[usize], device: &Device) {
    let t = Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
    affine.bias = Param::from_tensor(t);
}

/// 按目标名把一个命名 ONNX 权重填入解码器对应参数。
///
/// 目标名沿用 candle 版的参数位路径（如
/// `model.decoder.estimator.down_blocks.0.0.block1.block.0.weight`）。未知名字静默忽略，
/// 便于复用基座权重。
pub fn apply_named_decoder_weight(
    decoder: &mut BurnBaseDecoder,
    name: &str,
    data: &[f32],
    dims: &[usize],
    device: &Device,
) {
    let key = name
        .strip_prefix("model.decoder.estimator.")
        .unwrap_or(name);
    // `ff.net.0.alpha`/`ff.net.0.beta` 是参数化 GELU 常数，按 weight 语义处理。
    let alpha_beta = key.ends_with(".alpha") || key.ends_with(".beta");
    if !key.ends_with(".weight") && !key.ends_with(".bias") && !alpha_beta {
        return;
    }
    let is_weight = alpha_beta || key.ends_with(".weight");
    let base = key
        .strip_suffix(".weight")
        .or_else(|| key.strip_suffix(".bias"))
        .unwrap_or(key);

    // 找出该参数属哪个块（down/mid/up），以及块内的剩余子路径。
    let parse_block = |prefix: &str| -> Option<(usize, String)> {
        let rest = base.strip_prefix(prefix)?;
        let (i, after) = rest.split_once('.')?;
        i.parse().ok().map(|ix| (ix, after.to_string()))
    };

    // 时间 MLP（命名 Gemm，[out, in] 需转置）。
    if base == "time_mlp.linear_1" {
        if is_weight {
            set_linear_weight(&mut decoder.time_mlp.linear1, data, dims, true, device);
        } else {
            set_linear_bias(&mut decoder.time_mlp.linear1, data, dims, device);
        }
        return;
    }
    if base == "time_mlp.linear_2" {
        if is_weight {
            set_linear_weight(&mut decoder.time_mlp.linear2, data, dims, true, device);
        } else {
            set_linear_bias(&mut decoder.time_mlp.linear2, data, dims, device);
        }
        return;
    }
    // 收尾。final_block.block.0 是卷积，final_block.block.1 是归一化仿射，final_proj 卷积。
    if base.starts_with("final_block.block.0") {
        if is_weight {
            set_conv_weight(&mut decoder.final_block, data, dims, device);
        } else {
            set_conv_bias(&mut decoder.final_block, data, dims, device);
        }
        return;
    }
    if base.starts_with("final_block.block.1") {
        if is_weight {
            set_groups_weight(&mut decoder.final_norm, data, dims, device);
        } else {
            set_groups_bias(&mut decoder.final_norm, data, dims, device);
        }
        return;
    }
    if base == "final_proj" {
        if is_weight {
            set_conv_weight(&mut decoder.final_proj, data, dims, device);
        } else {
            set_conv_bias(&mut decoder.final_proj, data, dims, device);
        }
        return;
    }

    // 下行块：down_blocks.{i}.0=残差，.1.0=注意力，.2=采样卷积（i=0 名 `2.conv`，i=1 名 `2`）。
    if let Some((i, after)) = parse_block("down_blocks.") {
        let (res, attn, conv) = &mut decoder.down[i];
        // `after` 形如 `0.block1...` 或 `1.0.attn1...`，先剥离元组索引再分发给残差/注意力。
        if let Some(sub) = strip_tuple_index(&after) {
            apply_block(res, attn, sub, is_weight, data, dims, device);
        }
        // down 卷积：i=0 用 `2.conv`，i=1 用 `2`。用完整 `key`（含 weight/bias 后缀）匹配。
        let conv_key = if i == 0 { "2.conv" } else { "2" };
        let conv_name = key.strip_prefix(&format!("down_blocks.{i}.")).unwrap_or("");
        if conv_name.starts_with(conv_key) {
            if is_weight {
                set_conv_weight(conv, data, dims, device);
            } else {
                set_conv_bias(conv, data, dims, device);
            }
        }
        return;
    }

    // 中部块：mid_blocks.{i}.0=残差，.1.0=注意力，无采样卷积。
    if let Some((i, after)) = parse_block("mid_blocks.") {
        let (res, attn) = &mut decoder.mid[i];
        if let Some(sub) = strip_tuple_index(&after) {
            apply_block(res, attn, sub, is_weight, data, dims, device);
        }
        return;
    }

    // 上行块：up_blocks.{i}.0=残差，.1.0=注意力，.2=上采样（i=0 转置 `2.conv`，i=1 卷积 `2`）。
    if let Some((i, after)) = parse_block("up_blocks.") {
        let conv_name = key.strip_prefix(&format!("up_blocks.{i}.")).unwrap_or("");
        let (res, attn, sampler) = &mut decoder.up[i];
        if let Some(sub) = strip_tuple_index(&after) {
            apply_block(res, attn, sub, is_weight, data, dims, device);
        }
        // 采样/转置卷积名：i=0 用 `2.conv`，i=1 用 `2`。
        if conv_name.starts_with("2") {
            apply_sampler(sampler, is_weight, data, dims, device);
        }
    }
}

/// 从块的 ONNX 子路径里剥离开头的元组索引。
///
/// 块内是一个元组：`0.` 开头是残差块、`1.0.` 开头是注意力块。返回剥掉
/// 该前缀后的子路径（`block1...`、`mlp.1...`、`attn1...` 等）。不匹配返回 None。
fn strip_tuple_index(after: &str) -> Option<String> {
    if let Some(rest) = after.strip_prefix("0.") {
        return Some(rest.to_string());
    }
    // 注意力块在元组索引 1 下，再进一层索引 0。
    if let Some(rest) = after.strip_prefix("1.0.") {
        return Some(rest.to_string());
    }
    None
}

/// 把剩余子路径分发给残差块与注意力块。
fn apply_block(
    res: &mut ResBlock,
    attn: &mut AttnBlock,
    after: String,
    is_weight: bool,
    data: &[f32],
    dims: &[usize],
    device: &Device,
) {
    // 残差块：block1/block2 的 block.0=卷积、block.1=归一化仿射，res_conv，mlp.1=时间投影。
    if after.starts_with("block1.block.0") {
        if is_weight {
            set_conv_weight(&mut res.conv1, data, dims, device);
        } else {
            set_conv_bias(&mut res.conv1, data, dims, device);
        }
        return;
    }
    if after.starts_with("block2.block.0") {
        if is_weight {
            set_conv_weight(&mut res.conv2, data, dims, device);
        } else {
            set_conv_bias(&mut res.conv2, data, dims, device);
        }
        return;
    }
    if after.starts_with("res_conv") {
        if is_weight {
            set_conv_weight(&mut res.res_conv, data, dims, device);
        } else {
            set_conv_bias(&mut res.res_conv, data, dims, device);
        }
        return;
    }
    if after.starts_with("block1.block.1") {
        if is_weight {
            set_groups_weight(&mut res.norm1, data, dims, device);
        } else {
            set_groups_bias(&mut res.norm1, data, dims, device);
        }
        return;
    }
    if after.starts_with("block2.block.1") {
        if is_weight {
            set_groups_weight(&mut res.norm2, data, dims, device);
        } else {
            set_groups_bias(&mut res.norm2, data, dims, device);
        }
        return;
    }
    if after.starts_with("mlp.1") {
        if is_weight {
            set_linear_weight(&mut res.time_proj, data, dims, true, device);
        } else {
            set_linear_bias(&mut res.time_proj, data, dims, device);
        }
        return;
    }

    // 注意力块：norm1/norm3 仿射，attn1 的 qkv 权重（MatMul 匿名，不转）、to_out.0，ff。
    for (name, g) in [("norm1", true), ("norm3", false)] {
        if after.starts_with(name) {
            let a = if g { &mut attn.norm1 } else { &mut attn.norm3 };
            if is_weight {
                set_affine(a, data, dims, device);
            } else {
                set_affine_bias(a, data, dims, device);
            }
            return;
        }
    }
    if after.starts_with("attn1.to_q") {
        if is_weight {
            set_linear_weight(&mut attn.q, data, dims, false, device);
        }
        return;
    }
    if after.starts_with("attn1.to_k") {
        if is_weight {
            set_linear_weight(&mut attn.k, data, dims, false, device);
        }
        return;
    }
    if after.starts_with("attn1.to_v") {
        if is_weight {
            set_linear_weight(&mut attn.v, data, dims, false, device);
        }
        return;
    }
    if after.starts_with("attn1.to_out.0") {
        if is_weight {
            set_linear_weight(&mut attn.to_out, data, dims, false, device);
        } else {
            set_linear_bias(&mut attn.to_out, data, dims, device);
        }
        return;
    }
    if after.starts_with("ff.net.0.proj") {
        if is_weight {
            set_linear_weight(&mut attn.ff_proj, data, dims, false, device);
        } else {
            set_linear_bias(&mut attn.ff_proj, data, dims, device);
        }
        return;
    }
    if after.starts_with("ff.net.0.alpha") {
        if is_weight {
            let t = Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
            attn.ff_alpha = Param::from_tensor(t);
        }
        return;
    }
    if after.starts_with("ff.net.0.beta") {
        if is_weight {
            let t = Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
            attn.ff_beta = Param::from_tensor(t);
        }
        return;
    }
    if after.starts_with("ff.net.2") {
        if is_weight {
            set_linear_weight(&mut attn.ff_out, data, dims, false, device);
        } else {
            set_linear_bias(&mut attn.ff_out, data, dims, device);
        }
    }
}

/// 给上采样算子填权重/偏置。
fn apply_sampler(
    sampler: &mut UpSampler,
    is_weight: bool,
    data: &[f32],
    dims: &[usize],
    device: &Device,
) {
    match sampler {
        UpSampler::Transpose(ct) => {
            // 转置卷积权重 [c_out, c_in, k] -> burn [c_in, c_out, k]。
            if is_weight {
                let t =
                    Tensor::<3>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
                ct.weight = Param::from_tensor(t);
            } else if let Some(b) = ct.bias.as_mut() {
                let bt =
                    Tensor::<1>::from_data(TensorData::new(data.to_vec(), dims.to_vec()), device);
                *b = Param::from_tensor(bt);
            }
        },
        UpSampler::Conv(c) => {
            if is_weight {
                set_conv_weight(c, data, dims, device);
            } else {
                set_conv_bias(c, data, dims, device);
            }
        },
    }
}

// —— 以下为导出用回读（与 apply_named_decoder_weight 的写入方向互逆）——。

/// 把 burn 线性权重转回 ONNX 布局的扁平 f32。`transposed` 表示写入时 ONNX [out,in]
/// 被转置成 burn [in,out]，回读时对称地再转置回去。
fn read_linear_weight(linear: &burn::nn::Linear, transposed: bool) -> Option<Vec<f32>> {
    let t = if transposed {
        linear.weight.val().permute([1, 0])
    } else {
        linear.weight.val()
    };
    t.to_data().try_to_vec::<f32>().ok()
}

fn read_linear_bias(linear: &burn::nn::Linear) -> Option<Vec<f32>> {
    linear
        .bias
        .as_ref()?
        .val()
        .to_data()
        .try_to_vec::<f32>()
        .ok()
}

fn read_conv_weight(weight: &Param<Tensor<3>>) -> Option<Vec<f32>> {
    weight.val().to_data().try_to_vec::<f32>().ok()
}

fn read_conv_bias(conv: &Option<Param<Tensor<1>>>) -> Option<Vec<f32>> {
    conv.as_ref()?.val().to_data().try_to_vec::<f32>().ok()
}

fn read_groups_weight(group: &GroupNormAffine) -> Option<Vec<f32>> {
    group.weight.val().to_data().try_to_vec::<f32>().ok()
}

fn read_groups_bias(group: &GroupNormAffine) -> Option<Vec<f32>> {
    group.bias.val().to_data().try_to_vec::<f32>().ok()
}

fn read_affine_weight(affine: &Affine) -> Option<Vec<f32>> {
    affine.weight.val().to_data().try_to_vec::<f32>().ok()
}

fn read_affine_bias(affine: &Affine) -> Option<Vec<f32>> {
    affine.bias.val().to_data().try_to_vec::<f32>().ok()
}

/// 从解码器读出某残差/注意力块的权重，按 ONNX 语义名分发。
fn read_block_weight(
    res: &ResBlock,
    attn: &AttnBlock,
    after: &str,
    is_weight: bool,
) -> Option<Vec<f32>> {
    if after.starts_with("block1.block.0") {
        return if is_weight {
            read_conv_weight(&res.conv1.weight)
        } else {
            read_conv_bias(&res.conv1.bias)
        };
    }
    if after.starts_with("block2.block.0") {
        return if is_weight {
            read_conv_weight(&res.conv2.weight)
        } else {
            read_conv_bias(&res.conv2.bias)
        };
    }
    if after.starts_with("res_conv") {
        return if is_weight {
            read_conv_weight(&res.res_conv.weight)
        } else {
            read_conv_bias(&res.res_conv.bias)
        };
    }
    if after.starts_with("block1.block.1") {
        return if is_weight {
            read_groups_weight(&res.norm1)
        } else {
            read_groups_bias(&res.norm1)
        };
    }
    if after.starts_with("block2.block.1") {
        return if is_weight {
            read_groups_weight(&res.norm2)
        } else {
            read_groups_bias(&res.norm2)
        };
    }
    if after.starts_with("mlp.1") {
        return if is_weight {
            read_linear_weight(&res.time_proj, true)
        } else {
            read_linear_bias(&res.time_proj)
        };
    }
    // 注意力块。
    for (name, g) in [("norm1", true), ("norm3", false)] {
        if after.starts_with(name) {
            let a = if g { &attn.norm1 } else { &attn.norm3 };
            return if is_weight {
                read_affine_weight(a)
            } else {
                read_affine_bias(a)
            };
        }
    }
    if after.starts_with("attn1.to_q") {
        return if is_weight {
            read_linear_weight(&attn.q, false)
        } else {
            None
        };
    }
    if after.starts_with("attn1.to_k") {
        return if is_weight {
            read_linear_weight(&attn.k, false)
        } else {
            None
        };
    }
    if after.starts_with("attn1.to_v") {
        return if is_weight {
            read_linear_weight(&attn.v, false)
        } else {
            None
        };
    }
    if after.starts_with("attn1.to_out.0") {
        return if is_weight {
            read_linear_weight(&attn.to_out, false)
        } else {
            read_linear_bias(&attn.to_out)
        };
    }
    if after.starts_with("ff.net.0.proj") {
        return if is_weight {
            read_linear_weight(&attn.ff_proj, false)
        } else {
            read_linear_bias(&attn.ff_proj)
        };
    }
    if after.starts_with("ff.net.0.alpha") {
        return if is_weight {
            attn.ff_alpha.val().to_data().try_to_vec::<f32>().ok()
        } else {
            None
        };
    }
    if after.starts_with("ff.net.0.beta") {
        return if is_weight {
            attn.ff_beta.val().to_data().try_to_vec::<f32>().ok()
        } else {
            None
        };
    }
    if after.starts_with("ff.net.2") {
        return if is_weight {
            read_linear_weight(&attn.ff_out, false)
        } else {
            read_linear_bias(&attn.ff_out)
        };
    }
    None
}

/// 读上采样算子（conv 权重/偏置，转置卷积权重）。
fn read_sampler_weight(sampler: &UpSampler, is_weight: bool) -> Option<Vec<f32>> {
    match sampler {
        UpSampler::Transpose(ct) => {
            if is_weight {
                ct.weight.val().to_data().try_to_vec::<f32>().ok()
            } else {
                ct.bias.as_ref()?.val().to_data().try_to_vec::<f32>().ok()
            }
        },
        UpSampler::Conv(c) => {
            if is_weight {
                read_conv_weight(&c.weight)
            } else {
                read_conv_bias(&c.bias)
            }
        },
    }
}

/// 按 ONNX 语义名回读解码器某参数，返回 ONNX 布局的扁平 f32 值。
///
/// `name` 形如 `model.decoder.estimator.up_blocks.0.2.conv.weight`，与
/// `apply_named_decoder_weight` 的写入目标一致。找不到对应字段返回 None。用于把
/// 微调后的解码器权重导出回基座 ONNX。
pub fn read_decoder_param(decoder: &BurnBaseDecoder, name: &str) -> Option<Vec<f32>> {
    let base = name.strip_prefix("model.decoder.estimator.")?;
    // `ff.net.0.alpha`/`ff.net.0.beta` 是单值参数，按 weight 语义处理且保留完整子路径。
    let alpha_beta = base.ends_with(".alpha") || base.ends_with(".beta");
    let is_weight = alpha_beta || base.ends_with(".weight");
    let path = if alpha_beta {
        base
    } else if is_weight {
        base.strip_suffix(".weight")?
    } else if base.ends_with(".bias") {
        base.strip_suffix(".bias")?
    } else {
        return None;
    };

    if path == "time_mlp.linear_1" {
        return if is_weight {
            read_linear_weight(&decoder.time_mlp.linear1, true)
        } else {
            read_linear_bias(&decoder.time_mlp.linear1)
        };
    }
    if path == "time_mlp.linear_2" {
        return if is_weight {
            read_linear_weight(&decoder.time_mlp.linear2, true)
        } else {
            read_linear_bias(&decoder.time_mlp.linear2)
        };
    }
    if path.starts_with("final_block.block.0") {
        return if is_weight {
            read_conv_weight(&decoder.final_block.weight)
        } else {
            read_conv_bias(&decoder.final_block.bias)
        };
    }
    if path.starts_with("final_block.block.1") {
        return if is_weight {
            read_groups_weight(&decoder.final_norm)
        } else {
            read_groups_bias(&decoder.final_norm)
        };
    }
    if path == "final_proj" {
        return if is_weight {
            read_conv_weight(&decoder.final_proj.weight)
        } else {
            read_conv_bias(&decoder.final_proj.bias)
        };
    }

    let parse_block = |prefix: &str| -> Option<(usize, String)> {
        let rest = path.strip_prefix(prefix)?;
        let (i, after) = rest.split_once('.')?;
        i.parse().ok().map(|ix| (ix, after.to_string()))
    };

    if let Some((i, after)) = parse_block("down_blocks.") {
        let (res, attn, conv) = &decoder.down[i];
        let mut got = None;
        if let Some(sub) = strip_tuple_index(&after) {
            got = read_block_weight(res, attn, &sub, is_weight);
        }
        if got.is_some() {
            return got;
        }
        // down 卷积：i=0 用 `2.conv`，i=1 用 `2`。用完整 `base`（含 weight/bias 后缀）匹配，
        // 与写入侧一致。
        let conv_key = format!("2.{}", if i == 0 { "conv." } else { "" });
        if base
            .strip_prefix(&format!("down_blocks.{i}."))
            .map_or(false, |n| n.starts_with(&conv_key))
        {
            return if is_weight {
                read_conv_weight(&conv.weight)
            } else {
                read_conv_bias(&conv.bias)
            };
        }
        return None;
    }
    if let Some((i, after)) = parse_block("mid_blocks.") {
        let (res, attn) = &decoder.mid[i];
        if let Some(sub) = strip_tuple_index(&after) {
            return read_block_weight(res, attn, &sub, is_weight);
        }
        return None;
    }
    if let Some((i, after)) = parse_block("up_blocks.") {
        let (res, attn, sampler) = &decoder.up[i];
        if let Some(sub) = strip_tuple_index(&after) {
            let got = read_block_weight(res, attn, &sub, is_weight);
            if got.is_some() {
                return got;
            }
        }
        let conv_name = base.strip_prefix(&format!("up_blocks.{i}."));
        if conv_name.map_or(false, |n| n.starts_with("2")) {
            return read_sampler_weight(sampler, is_weight);
        }
        return None;
    }
    None
}

/// 从基座 ONNX 权重构建解码器：先填命名张量（卷积、线性、偏置、时间 MLP、注意力仿射、
/// ff 常数），再按 decoder_weight_map 把匿名常量（注意力 qkv 权重、归一化仿射）落位，
/// 其余保留随机初值。
///
/// `tensors` 来自 onnx_weights::read_initializers。地图文件用 candle 版的
/// `decoder_weight_map.json`。
pub fn from_onnx_tensors(
    config: &BurnBaseDecoderConfig,
    tensors: &[super::onnx_weights::OnnxTensor],
    map_path: &std::path::Path,
    device: &Device,
) -> Result<BurnBaseDecoder, String> {
    use std::collections::HashMap;

    let mut decoder = BurnBaseDecoder::init(config, device);
    // 命名张量：只处理 decoder.estimator 路径下的 weight/bias。
    for tensor in tensors {
        if tensor.name.starts_with("model.decoder.estimator.") {
            apply_named_decoder_weight(
                &mut decoder,
                &tensor.name,
                &tensor.data,
                &tensor.dims,
                device,
            );
        }
    }

    // 匿名常量：按 decoder_weight_map.json 的 target 路径落位。目标名与命名张量一致，
    // 调用同一个分发逻辑；MatMul 布局（[in, out]）已在分发层处理。
    let bytes = std::fs::read(map_path).map_err(|e| format!("读取映射表失败: {e}"))?;
    let map: HashMap<String, serde_json::Value> =
        serde_json::from_slice(&bytes).map_err(|e| format!("解析映射表失败: {e}"))?;
    let by_name: HashMap<&str, &super::onnx_weights::OnnxTensor> =
        tensors.iter().map(|t| (t.name.as_str(), t)).collect();

    let mut bound = 0usize;
    for (src, info) in &map {
        let Some(target) = info.get("target").and_then(|v| v.as_str()) else {
            continue;
        };
        // 只处理解码器路径；编码器匿名归一化由 base_encoder 单独处理。
        if !target.starts_with("model.decoder.") {
            continue;
        }
        let target = target.replace('/', ".");
        let Some(tensor) = by_name.get(src.as_str()) else {
            continue;
        };
        apply_named_decoder_weight(&mut decoder, &target, &tensor.data, &tensor.dims, device);
        bound += 1;
    }

    if bound == 0 {
        return Err("decoder 未绑定任何匿名权重".to_string());
    }
    Ok(decoder)
}

#[cfg(test)]
mod tests {
    use super::super::burn_backend::{train_device, train_device_autodiff};
    use super::*;

    /// `no_grad` 后按路径解冻时，只应恢复 FiLM 与卷积旁路参数。
    #[test]
    fn adapter_groups_only_unfreeze_adapters() {
        let device = train_device_autodiff();
        let config = BurnBaseDecoderConfig {
            conv_lora_rank: 2,
            ..Default::default()
        };
        let decoder = BurnBaseDecoder::init(&config, &device)
            .no_grad()
            .set_require_grad_group(burn::module::ParamGroup::from_predicate("style_film"), true)
            .set_require_grad_group(burn::module::ParamGroup::from_predicate("conv_lora"), true);
        assert!(decoder.final_norm.style_film.gamma.is_require_grad());
        assert!(decoder.final_norm.style_film.beta.is_require_grad());
        assert!(!decoder.final_norm.weight.is_require_grad());
        assert!(!decoder.final_proj.weight.is_require_grad());
        let lora = decoder.down[0].0.conv_lora1.as_ref().unwrap();
        assert!(lora.down.weight.is_require_grad());
        assert!(lora.up.weight.is_require_grad());
        assert!(!decoder.down[0].2.weight.is_require_grad());
        assert!(
            decoder.down_conv_lora[0]
                .as_ref()
                .unwrap()
                .down
                .weight
                .is_require_grad()
        );
        match decoder.up_conv_lora[0].as_ref().unwrap() {
            SamplerLora::Transpose(lora) => {
                assert!(lora.down.weight.is_require_grad());
                assert!(lora.up.weight.is_require_grad());
            },
            SamplerLora::Conv(_) => panic!("第一个上采样层应为转置卷积"),
        }
        assert!(
            decoder
                .final_block_conv_lora
                .as_ref()
                .unwrap()
                .up
                .weight
                .is_require_grad()
        );
        assert!(
            decoder
                .final_proj_conv_lora
                .as_ref()
                .unwrap()
                .down
                .weight
                .is_require_grad()
        );
        assert!(!decoder.down[0].0.conv1.weight.is_require_grad());
    }

    /// FiLM 折叠成归一化仿射后，前向结果必须逐元素等价。
    #[test]
    fn style_film_fold_matches_original_forward() {
        let device = train_device();

        let mut group = GroupNormAffine::new(4, 2, &device);
        group.weight = Param::from_tensor(Tensor::<2>::from_data(
            TensorData::new(vec![0.5f32, 1.2, 0.8, 1.5], [4, 1]),
            &device,
        ));
        group.bias = Param::from_tensor(Tensor::<2>::from_data(
            TensorData::new(vec![0.1f32, -0.2, 0.3, 0.0], [4, 1]),
            &device,
        ));
        group.style_film.gamma = Param::from_tensor(Tensor::<1>::from_data(
            TensorData::new(vec![1.3f32, 0.7, 1.1, 0.9], [4]),
            &device,
        ));
        group.style_film.beta = Param::from_tensor(Tensor::<1>::from_data(
            TensorData::new(vec![0.2f32, -0.1, 0.05, 0.4], [4]),
            &device,
        ));
        let xs = Tensor::<3>::from_data(
            TensorData::new(
                (0..24).map(|i| i as f32 * 0.1 - 1.0).collect::<Vec<f32>>(),
                [1, 4, 6],
            ),
            &device,
        );
        let before = group.forward(xs.clone());
        group.fold_style_film();
        let after = group.forward(xs);
        let diff: Vec<f32> = (before - after)
            .abs()
            .to_data()
            .try_to_vec::<f32>()
            .unwrap();
        assert!(
            diff.iter().all(|v| *v < 1e-5),
            "group norm 折叠误差过大: {diff:?}"
        );

        let mut affine = Affine::new(4, &device);
        affine.weight = Param::from_tensor(Tensor::<1>::from_data(
            TensorData::new(vec![0.6f32, 1.4, 0.9, 1.2], [4]),
            &device,
        ));
        affine.bias = Param::from_tensor(Tensor::<1>::from_data(
            TensorData::new(vec![-0.1f32, 0.2, 0.0, 0.3], [4]),
            &device,
        ));
        affine.style_film.gamma = Param::from_tensor(Tensor::<1>::from_data(
            TensorData::new(vec![1.1f32, 0.8, 1.4, 0.95], [4]),
            &device,
        ));
        affine.style_film.beta = Param::from_tensor(Tensor::<1>::from_data(
            TensorData::new(vec![0.15f32, -0.05, 0.25, -0.2], [4]),
            &device,
        ));
        let xs = Tensor::<3>::from_data(
            TensorData::new(
                (0..24)
                    .map(|i| (i as f32 * 0.3).sin())
                    .collect::<Vec<f32>>(),
                [1, 4, 6],
            ),
            &device,
        );
        let before = affine.forward(xs.clone());
        affine.fold_style_film();
        let after = affine.forward(xs);
        let diff: Vec<f32> = (before - after)
            .abs()
            .to_data()
            .try_to_vec::<f32>()
            .unwrap();
        assert!(
            diff.iter().all(|v| *v < 1e-5),
            "affine 折叠误差过大: {diff:?}"
        );
    }

    /// 卷积 LoRA 折进基座卷积后，前向结果必须逐元素等价。
    #[test]
    fn conv_lora_fold_matches_original_forward() {
        let device = train_device();
        let base = Conv1dConfig::new(3, 4, 3)
            .with_padding(PaddingConfig1d::Explicit(1, 1))
            .init(&device);
        let mut lora = ConvLora::new(3, 4, 3, 1, 1, 2, &device);
        lora.down.weight = Param::from_tensor(Tensor::<3>::random(
            [2, 3, 1],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        ));
        lora.up.weight = Param::from_tensor(Tensor::<3>::random(
            [4, 2, 3],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        ));
        let xs = Tensor::<3>::random(
            [1, 3, 7],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        );
        let expected = base.forward(xs.clone()) + lora.forward(xs.clone());
        let mut folded = base.clone();
        lora.fold_into(&mut folded);
        let actual = folded.forward(xs);
        let diff: Vec<f32> = (expected - actual)
            .abs()
            .to_data()
            .try_to_vec::<f32>()
            .unwrap();
        assert!(
            diff.iter().all(|value| *value < 1e-5),
            "卷积 LoRA 折叠误差过大: {diff:?}"
        );
    }

    /// 导出副本应移除卷积旁路，避免折叠后重复施加。
    #[test]
    fn folded_export_removes_conv_lora() {
        let device = train_device();
        let config = BurnBaseDecoderConfig {
            conv_lora_rank: 2,
            ..Default::default()
        };
        let decoder = BurnBaseDecoder::init(&config, &device);
        assert!(decoder.down[0].0.conv_lora1.is_some());
        assert!(decoder.down_conv_lora[0].is_some());
        assert!(decoder.up_conv_lora.iter().all(Option::is_some));
        assert!(decoder.final_block_conv_lora.is_some());
        assert!(decoder.final_proj_conv_lora.is_some());
        let folded = decoder.folded_for_export();
        assert!(folded.down[0].0.conv_lora1.is_none());
        assert!(folded.mid[0].0.conv_lora2.is_none());
        assert!(folded.up[0].0.conv_lora_res.is_none());
        assert!(folded.down_conv_lora.iter().all(Option::is_none));
        assert!(folded.up_conv_lora.iter().all(Option::is_none));
        assert!(folded.final_block_conv_lora.is_none());
        assert!(folded.final_proj_conv_lora.is_none());
    }

    /// 在当前平台设备上跑一次前向，验证形状与链路可用。
    /// 输入 [1, 160, 5]，时间 [1, 160]，输出 [1, 80, 10]（上采样后帧数变化）。
    #[test]
    fn forward_produces_velocity_field() {
        let device = train_device();
        let config = BurnBaseDecoderConfig::default();
        let decoder = BurnBaseDecoder::init(&config, &device);

        let noisy: Tensor<3, Float> = Tensor::<3, Float>::from_data(
            TensorData::new(vec![0.0f32; 80 * 5], vec![1, 80, 5]),
            &device,
        );
        let text: Tensor<3, Float> = Tensor::<3, Float>::from_data(
            TensorData::new(vec![0.0f32; 80 * 5], vec![1, 80, 5]),
            &device,
        );
        let time: Tensor<2, Float> = Tensor::<2, Float>::from_data(
            TensorData::new(vec![0.5f32; 160], vec![1, 160]),
            &device,
        );
        let out: Tensor<3, Float> = decoder.forward(noisy, text, time);
        // 下行把 5 帧变 3，上行转置再放大，最终约 [1, 80, N]。仅校验通道数。
        assert_eq!(out.shape().dims::<3>()[1], 80);
    }

    /// 从基座 ONNX 加载权重到 burn 解码器，验证权重桥可用且前向形状正确。
    #[test]
    fn loads_from_real_onnx_and_forwards() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx");
        if !path.exists() {
            return;
        }
        let map_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/tts-training/decoder_weight_map.json");
        let tensors = super::super::onnx_weights::read_initializers(&path).unwrap();
        let device = train_device();
        let config = BurnBaseDecoderConfig::default();
        let decoder = from_onnx_tensors(&config, &tensors, &map_path, &device).unwrap();

        let noisy: Tensor<3, Float> = Tensor::<3, Float>::from_data(
            TensorData::new(vec![0.0f32; 80 * 5], vec![1, 80, 5]),
            &device,
        );
        let text: Tensor<3, Float> = Tensor::<3, Float>::from_data(
            TensorData::new(vec![0.0f32; 80 * 5], vec![1, 80, 5]),
            &device,
        );
        let time: Tensor<2, Float> = Tensor::<2, Float>::from_data(
            TensorData::new(vec![0.5f32; 160], vec![1, 160]),
            &device,
        );
        let out: Tensor<3, Float> = decoder.forward(noisy, text, time);
        assert_eq!(out.shape().dims::<3>()[1], 80);
    }

    /// 从基座 ONNX 加载解码器后逐一回读，确认与原始 ONNX 权重一致（导出逆桥正确）。
    #[test]
    fn read_back_matches_onnx_original() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker/model-steps-3.onnx");
        if !path.exists() {
            return;
        }
        let map_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/tts-training/decoder_weight_map.json");
        let tensors = super::super::onnx_weights::read_initializers(&path).unwrap();
        let device = train_device();
        let config = BurnBaseDecoderConfig::default();
        let decoder = from_onnx_tensors(&config, &tensors, &map_path, &device).unwrap();

        let by_name: std::collections::HashMap<&str, &super::super::onnx_weights::OnnxTensor> =
            tensors.iter().map(|t| (t.name.as_str(), t)).collect();

        let mut checked = 0usize;
        // 命名张量：所有 decoder.estimator 路径下权重均应能回读且一致。
        for tensor in &tensors {
            if !tensor.name.starts_with("model.decoder.estimator.") {
                continue;
            }
            let read = read_decoder_param(&decoder, &tensor.name)
                .unwrap_or_else(|| panic!("回读不到 {}", tensor.name));
            assert_eq!(read, tensor.data, "{} 回读不一致", tensor.name);
            checked += 1;
        }
        // 匿名 MatMul 常量：从映射表取 target 属于 decoder 的，通过 target 语义名回读。
        let bytes = std::fs::read(&map_path).unwrap();
        let map: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_slice(&bytes).unwrap();
        for (src, info) in &map {
            let Some(target) = info.get("target").and_then(|v| v.as_str()) else {
                continue;
            };
            if !target.starts_with("model.decoder.") {
                continue;
            }
            let target = target.replace('/', ".");
            let Some(tensor) = by_name.get(src.as_str()) else {
                continue;
            };
            let read = read_decoder_param(&decoder, &target)
                .unwrap_or_else(|| panic!("回读不到匿名权重 {target}"));
            assert_eq!(read, tensor.data, "{target} 回读不一致");
            checked += 1;
        }
        assert!(checked >= 2, "未校验到任何 decoder 权重");
    }

    fn randomize_conv_lora(lora: &mut ConvLora, device: &Device) {
        lora.up.weight = Param::from_tensor(Tensor::<3>::random(
            lora.up.weight.val().dims(),
            burn::tensor::Distribution::Normal(0.0, 0.3),
            device,
        ));
    }

    /// 下采样、上采样（含转置卷积）与收尾投影的旁路都必须能精确折叠。
    #[test]
    fn folded_export_matches_forward_with_all_conv_lora() {
        let device = train_device();
        let config = BurnBaseDecoderConfig {
            channels: 8,
            time_dim: 8,
            time_hidden: 16,
            n_mels: 4,
            input_channels: 8,
            conv_lora_rank: 2,
        };
        let mut decoder = BurnBaseDecoder::init(&config, &device);

        for (block, _, _) in &mut decoder.down {
            for lora in [
                block.conv_lora1.as_mut(),
                block.conv_lora2.as_mut(),
                block.conv_lora_res.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                randomize_conv_lora(lora, &device);
            }
        }
        for lora in decoder.down_conv_lora.iter_mut().flatten() {
            randomize_conv_lora(lora, &device);
        }
        for lora in decoder.up_conv_lora.iter_mut().flatten() {
            match lora {
                SamplerLora::Transpose(lora) => {
                    lora.up.weight = Param::from_tensor(Tensor::<3>::random(
                        lora.up.weight.val().dims(),
                        burn::tensor::Distribution::Normal(0.0, 0.3),
                        &device,
                    ));
                },
                SamplerLora::Conv(lora) => randomize_conv_lora(lora, &device),
            }
        }
        randomize_conv_lora(decoder.final_block_conv_lora.as_mut().unwrap(), &device);
        randomize_conv_lora(decoder.final_proj_conv_lora.as_mut().unwrap(), &device);

        let noisy = Tensor::<3>::random(
            [1, config.n_mels, 6],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        );
        let text = Tensor::<3>::random(
            [1, config.n_mels, 6],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        );
        let time = Tensor::<2>::random(
            [1, config.time_dim],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        );
        let expected = decoder.forward(noisy.clone(), text.clone(), time.clone());
        let folded = decoder.folded_for_export();
        let actual = folded.forward(noisy, text, time);
        let diff: Vec<f32> = (expected - actual)
            .abs()
            .to_data()
            .try_to_vec::<f32>()
            .unwrap();
        assert!(
            diff.iter().all(|value| *value < 1e-4),
            "全卷积旁路折叠误差过大: {diff:?}"
        );
    }
}
