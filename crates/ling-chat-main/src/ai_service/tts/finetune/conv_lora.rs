//! 一维卷积的低秩旁路（Conv LoRA），解码器与编码器时长分支共用。
//!
//! 步长放在上投影，下投影保持 1x1；上投影零初始化，训练起点等于基座。
//! 转置卷积的下投影必须 `stride=1 / padding=0`，否则复合有效步长改变，无法折回基座。

use burn::module::{Module, Param};
use burn::nn::PaddingConfig1d;
use burn::nn::conv::{Conv1d, Conv1dConfig, ConvTranspose1d, ConvTranspose1dConfig};
use burn::prelude::*;

/// 普通一维卷积的低秩旁路。
#[derive(Module, Debug)]
pub(crate) struct ConvLora {
    pub(crate) down: Conv1d,
    pub(crate) up: Conv1d,
    scale: f32,
}

impl ConvLora {
    /// 构造与基座卷积同形状的旁路；`stride`、`padding` 必须与基座一致。
    pub(crate) fn new(
        input_channels: usize,
        output_channels: usize,
        kernel: usize,
        stride: usize,
        padding: usize,
        rank: usize,
        device: &Device,
    ) -> Self {
        let rank = rank.max(1);
        let down = Conv1dConfig::new(input_channels, rank, 1)
            .with_bias(false)
            .init(device);
        let mut up = Conv1dConfig::new(rank, output_channels, kernel)
            .with_stride(stride)
            .with_padding(PaddingConfig1d::Explicit(padding, padding))
            .with_bias(false)
            .init(device);
        // 零初始化上投影，保证训练起点严格等价于基座。
        up.weight = Param::from_tensor(Tensor::<3>::zeros(up.weight.val().dims(), device));
        Self {
            down,
            up,
            scale: 1.0,
        }
    }

    pub(crate) fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        self.up.forward(self.down.forward(xs)) * self.scale
    }

    /// 把低秩增量折进基座卷积权重。
    pub(crate) fn fold_into(&self, base: &mut Conv1d) {
        let [rank, in_channels, _] = self.down.weight.val().dims();
        let [out_channels, _, kernel] = self.up.weight.val().dims();
        let a = self.down.weight.val().reshape([rank, in_channels]);
        let b = self.up.weight.val().reshape([out_channels, rank, kernel]);
        let mut kernels = Vec::with_capacity(kernel);
        for index in 0..kernel {
            let bk = b.clone().narrow(2, index, 1).reshape([out_channels, rank]);
            kernels.push(bk.matmul(a.clone()).reshape([out_channels, in_channels, 1]));
        }
        let delta = Tensor::cat(kernels, 2) * self.scale;
        base.weight = Param::from_tensor(base.weight.val() + delta);
    }
}

/// 一维转置卷积的低秩旁路。
#[derive(Module, Debug)]
pub(crate) struct ConvTransposeLora {
    pub(crate) down: ConvTranspose1d,
    pub(crate) up: ConvTranspose1d,
    scale: f32,
}

impl ConvTransposeLora {
    pub(crate) fn new(
        input_channels: usize,
        output_channels: usize,
        kernel: usize,
        stride: usize,
        padding: usize,
        rank: usize,
        device: &Device,
    ) -> Self {
        let rank = rank.max(1);
        // 下投影必须是 stride=1 / padding=0 的 1x1 转置卷积，否则无法折回基座。
        let down = ConvTranspose1dConfig::new([input_channels, rank], 1)
            .with_stride(1)
            .with_padding(0)
            .with_bias(false)
            .init(device);
        let mut up = ConvTranspose1dConfig::new([rank, output_channels], kernel)
            .with_stride(stride)
            .with_padding(padding)
            .with_bias(false)
            .init(device);
        up.weight = Param::from_tensor(Tensor::<3>::zeros(up.weight.val().dims(), device));
        Self {
            down,
            up,
            scale: 1.0,
        }
    }

    pub(crate) fn forward(&self, xs: Tensor<3>) -> Tensor<3> {
        self.up.forward(self.down.forward(xs)) * self.scale
    }

    /// 把低秩增量折进基座转置卷积权重；burn 与 ONNX 的权重布局一致，可直接覆写导出。
    pub(crate) fn fold_into(&self, base: &mut ConvTranspose1d) {
        let [in_channels, rank, _] = self.down.weight.val().dims();
        let [_, out_channels, kernel] = self.up.weight.val().dims();
        let a = self.down.weight.val().reshape([in_channels, rank]);
        let b = self.up.weight.val().reshape([rank, out_channels, kernel]);
        let mut kernels = Vec::with_capacity(kernel);
        for index in 0..kernel {
            let bk = b.clone().narrow(2, index, 1).reshape([rank, out_channels]);
            kernels.push(a.clone().matmul(bk).reshape([in_channels, out_channels, 1]));
        }
        let delta = Tensor::cat(kernels, 2) * self.scale;
        base.weight = Param::from_tensor(base.weight.val() + delta);
    }
}

#[cfg(test)]
mod tests {
    use super::super::burn_backend::train_device;
    use super::*;

    fn assert_close(expected: Tensor<3>, actual: Tensor<3>, label: &str) {
        let diff: Vec<f32> = (expected - actual)
            .abs()
            .to_data()
            .try_to_vec::<f32>()
            .unwrap();
        assert!(
            diff.iter().all(|value| *value < 1e-5),
            "{label} 折叠误差过大: {diff:?}"
        );
    }

    /// 步长 2 的下采样卷积也必须能精确折叠。
    #[test]
    fn conv_lora_fold_matches_strided_forward() {
        let device = train_device();
        let base = Conv1dConfig::new(3, 4, 3)
            .with_stride(2)
            .with_padding(PaddingConfig1d::Explicit(1, 1))
            .init(&device);
        let mut lora = ConvLora::new(3, 4, 3, 2, 1, 2, &device);
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
            [1, 3, 11],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        );
        let expected = base.forward(xs.clone()) + lora.forward(xs.clone());
        let mut folded = base.clone();
        lora.fold_into(&mut folded);
        assert_close(expected, folded.forward(xs), "步长 2 卷积 LoRA");
    }

    /// 转置卷积旁路的折叠必须与训练前向逐元素一致。
    #[test]
    fn conv_transpose_lora_fold_matches_forward() {
        let device = train_device();
        let base = ConvTranspose1dConfig::new([4, 4], 4)
            .with_stride(2)
            .with_padding(1)
            .init(&device);
        let mut lora = ConvTransposeLora::new(4, 4, 4, 2, 1, 2, &device);
        lora.down.weight = Param::from_tensor(Tensor::<3>::random(
            [4, 2, 1],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        ));
        lora.up.weight = Param::from_tensor(Tensor::<3>::random(
            [2, 4, 4],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        ));
        let xs = Tensor::<3>::random(
            [1, 4, 5],
            burn::tensor::Distribution::Normal(0.0, 0.5),
            &device,
        );
        let expected = base.forward(xs.clone()) + lora.forward(xs.clone());
        let mut folded = base.clone();
        lora.fold_into(&mut folded);
        assert_close(expected, folded.forward(xs), "转置卷积 LoRA");
    }
}
