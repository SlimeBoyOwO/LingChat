# 端侧音色微调：基座结构备忘

## 基座

`data/sherpa_onnx_models/matcha-icefall-zh-baker/`，来自 sherpa-onnx 官方发布包。

- `tokens.txt` / `lexicon.txt`：文本前端的符号表与词表。
- `model-steps-3.onnx`：305 个 f32 初始张量，约 18.57M 参数，4802 个节点。

图的输入输出（流匹配采样步数已烘进图里）：

```
x            文本 token id
x_length     有效长度
noise_scale  噪声强度
length_scale 时长缩放
mel          输出 mel
```

## 张量布局

编码器（89 个张量）：

```
model.encoder.emb.weight                    [2069, 192]
model.encoder.prenet.conv_layers.{0,1,2}    [192, 192, 5] + bias
model.encoder.prenet.proj                   [192, 192, 1] + bias
model.encoder.encoder.attn_layers.{0..5}.conv_{q,k,v,o}   [192, 192, 1] + bias
model.encoder.encoder.ffn_layers.{0..5}.conv_1            [768, 192, 3] + bias
model.encoder.encoder.ffn_layers.{0..5}.conv_2            [192, 768, 3] + bias
```

解码器（118 个张量）按前缀分为：

```
model.decoder.estimator.down_blocks    38
model.decoder.estimator.up_blocks      38
model.decoder.estimator.mid_blocks     34
model.decoder.estimator.time_mlp        4
model.decoder.estimator.final_block     2
model.decoder.estimator.final_proj      2
```

其余 98 个张量是匿名常量（`onnx::Mul_*` / `onnx::Add_*` 等）。它们不是丢失的权重，
而是导出时被拆散并重命名的参数，可以按形状和出现顺序确定性还原。

匿名张量形状分布：

```
30  [1, 192, 1]   编码器归一化的 weight/bias，成对出现，共 15 层
 6  [1024, 256]   解码器注意力块的前馈第一层
 6  [256, 1024]   解码器注意力块的前馈第二层
18  [256, 128]    解码器注意力的 q/k/v 投影，每块 3 个，共 6 块
 6  [128, 256]    解码器注意力的输出投影
26  [256, 1]      卷积残差块的调制参数，每块一对，共 13 块
 4  [1, 256, 1]   解码器收尾与时间 MLP 的归一化参数
 2  [1]           标量常量
```

图里没有 `LayerNormalization` 算子，被拆成 `ReduceMean → Sub → Pow → ReduceMean → Add →
Sqrt → Div → Mul → Add`，末尾两个常量就是仿射参数，这也是按顺序还原的依据。

解码器块结构（按命名张量归纳）：

```
time_mlp.linear_1/2        160 → 1024 → 1024
down_blocks.N.0            卷积残差块（含时间调制 mlp.1）
down_blocks.N.1.0          自注意力块（norm1/norm3、attn1、ff.net.0.alpha/beta）
down_blocks.N.2            下采样卷积
mid_blocks.0/1             同 down 的两种块
up_blocks.N.0              卷积残差块，输入通道 512（拼接跳连）
up_blocks.N.2              上采样卷积（kernel 4）
final_block / final_proj   收尾卷积与 mel 投影
```

## 移植顺序

1. 编码器：词嵌入 + prenet + 6 层（1x1 卷积投影的自注意力、k=3 的 FFN 卷积、前后归一化）。用真实权重做逐张量形状校验。
2. 时长预测器：位于编码器输出之后，需先从张量清单里确认其命名。
3. 解码器：down / mid / up 三种块加时间 MLP，结构按块内张量逐一还原。
4. 权重绑定：读取 initializer，按名字填入 candle 参数表；折平常量单独处理。
5. 导出：只导出声学部分（文本到 mel），采样循环放在 Rust 侧，避免把步数烘进图。

## 现状

- 权重读取已实现并对着真实文件验证（`finetune::onnx_weights`）。
- 训练侧的模型与解码器是简化结构，只能验证训练管线，装不进基座权重，移植时需要替换。
