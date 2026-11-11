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

## 当前实现

- `burn_encoder.rs` / `burn_decoder.rs`：按基座 ONNX 的权重名装载编码器、解码器；编码器主干默认冻结。
- `burn_training.rs`：CFM 对齐训练，固定 Vulkan（桌面/Android）或 Metal（Apple）autodiff，不回落 CPU；支持解码器 LoRA、`adapter_only` 风格 FiLM、逐 token 时长适配和断点续训。
- `conv_lora.rs`：卷积/转置卷积 LoRA，导出时折叠成等价卷积权重。
- `onnx_export.rs`：把解码器、风格 FiLM、LoRA、时长分支折叠后写回基座 ONNX，只替换 initializer，不改图结构。
- `import.rs` / `normalize.rs` / `feature.rs` / `style.rs` / `corpus.rs`：数据集导入、重采样与响度归一化、mel 特征、确定性 k-means 风格聚类、语料构建。
- `text.rs` / `onnx_weights.rs`：基座词表/lexicon 前端与 ONNX initializer 读取。

## 训练与导出流程

1. 导入数据集：解压音频，统一采样率与响度，校验文本和音频时长。
2. 风格聚类：默认 4 簇、最多 16 簇；每簇使用独立检查点和续训状态。
3. 构建语料：文本前端 tokenize，提取 mel，按基座时长预测得到对齐帧数。
4. 训练：编码器主干冻结，更新解码器；可选 LoRA、`adapter_only` 风格 FiLM；时长适配额外训练编码器时长分支的低秩旁路。
5. 导出：折叠 FiLM / Conv LoRA / 时长分支，写回基座 ONNX 的 initializer，生成模型管理器可识别的 matcha 模型目录。
6. 全程在 Rust 进程内完成，不依赖外部训练二进制；训练设备不走 CPU 回退。
