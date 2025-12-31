---
title: Style-Bert-VITS2 模型训练
description: 了解如何训练一个Style-Bert-VITS2 模型用于语音合成
---

# Style-Bert-VITS2 模型训练

> 在使用本教程之前，请确保已经正确部署 Style-Bert-VITS2
> 如果您是windows系统且为英伟达显卡，也尝试使用 [Style-Bert-VITS2一键包](https://www.modelscope.cn/models/lingchat-research-studio/Style-Bert-VITS2-micromamba-Full-v2.7/files) 实现快速部署

## Step 1 数据集的准备

Style-Bert-VITS2 的数据集要求你准备【🎹 音频数据】和【🖊 文本数据】

### 🎹 音频数据的要求

- 建议为 wav 格式的音频数据，且音频采样率为 44100hz
- **注意**：修改文件后缀名并不能真正改变文件的格式，请使用专门的工具！

### 🖊 文本数据的要求

- 请保证你的文本数据的文件名称为：`esd.list`
- 请保证你的格式正确，以下为具体格式的参考

```txt
data.wav|character|JP|text
```

以角色（character）为 Murasame 来举例，则格式如下

```txt
mur001_006.wav|Murasame|JP|だから吾輩は、幽霊ではないと言っておるであろう！
mur001_007.wav|Murasame|JP|大体、折れた程度で復讐する必要などない。叢雨丸ならばこの程度すぐに元に戻る
mur001_008.wav|Murasame|JP|百聞は一見にしかずとも言う。先に証明した方がよかろう
```

> 请注意文本数据集内空格的存在，可自行查阅互联网或借助 AI 工具来得知如何使用记事本工具去除空格
> ( 审核注解: ~~或者眼睛够尖也可以~~ )

### 数据集整理步骤

当你准备好【🎹 音频数据】和【🖊 文本数据】后：

1. 创建一个文件夹，其名称为 `raw` ，然后将所有的【🎹 音频数据】（wav 文件）放入其中
2. 再创建一个文件夹，文件夹名称需要与你在修改【🖊 文本数据】里 `character` 一节时，所修改的内容保持一致
3. 将`raw`文件夹和`esd.list`文件放入上一步创建的文件夹中
4. 最后将该文件夹放入 Style-Bert-VITS2 项目文件夹下的`Data`文件夹

#### 这里我们以上文 Murasame 为案例

1. 创建名为`raw`的文件夹，将`mur001_006.wav`、`mur001_007.wav`、`mur001_008.wav`等 wav 文件放入其中
2. 创建名为`Murasame`的文件夹（与`esd.list`中的 character 名称保持一致）
3. 将`esd.list`文件与`raw`文件夹一同放入`Murasame`文件夹中
4. 将`Murasame`文件夹放入 Style-Bert-VITS2 项目文件夹下的`Data`文件夹

具体各文件位置最后如下：

```txt
Data
└─ Murasame
   ├─ esd.list
   └─ raw
      ├─ mur001_006.wav
      └─ .....（其他音频）

```

**至此，你已经完成了数据集的全部准备**

## Step 2 训练模型

> [!NOTE]
> 注意：接下来的内容预设了你没有使用网页翻译工具，所以将会以日文为主，并且在括号内附带参考翻译
>
> 该项目的全部功能无法一个个指出，所以更多的内容请借助 AI 工具或者翻译工具自行查阅了解
>
> 除了建议的内容外，其余在第一次尝鲜时可以保持默认，在未来的多次尝试和工具帮助下你可以学习更多更好的去设置

### 第一步：启动训练程序

点击 Style-Bert-VITS2 项目文件夹下的`Train.bat`或者`App.bat`

- `Train.bat`：只打开 Style-Bert-VITS2 的训练功能，启动较快

- `App.bat`：打开 Style-Bert-VITS2 所有功能，启动较慢，你需要点击界面里的`学習`来进入到训练页

### 第二步：设置模型名称

在`モデル名`（模型名称）下填入你`esd.list`下的模型名称，以上文为例，填入`Murasame`

### 第三步：进行简单的设置

#### バッチサイズ（批处理大小）

```text
学習速度が遅い場合は小さくして試し、
VRAMに余裕があれば大きくしてください。
JP-Extra版でのVRAM使用量目安: 1: 6GB, 2: 8GB, 3: 10GB, 4: 12GB
```

翻译：

```txt
如果学习速度较慢，可以尝试调小该值；
若 VRAM 有富余则可调大。
JP-Extra 版的 VRAM 使用量参考：1: 6GB, 2: 8GB, 3: 10GB, 4: 12GB
```

**设置建议：**

- 根据电脑显存来设置
- 注意不要开到最大，而是设置最大承受值减一为推荐
- 依据纯粹经验，批处理大小的值每提高一点，显存占用增加2GB

#### 音声の音量を正規化する（对音频音量进行标准化处理）

大部分情况下都建议保持开启，但这只作为参考建议

如果你可以保证角色音频 **不是东拼西凑来的** 或者自己 **全部听过音量相同** ，则可以不开启

#### JP-Extra

如果您的人物语音均为日语且之后 **不需要合成中文语音** ，务必打开选项`JP-Extra版を使う`（使用 JP-Extra 版）以获得更好的日语语音生成

### 第四步：自动预处理

点击具有颜色的大按钮`自動前処理`（自动预处理）来让程序对自己准备好的数据集进行处理

**处理完成后将显示：**

```text
Success: 全ての前処理が完了しました。ターミナルを確認しておかしいところがないか確認するのをおすすめします。
```

### 第五步：开始训练

- 拉到最底下，点击具有颜色的按钮 `学習を開始する`（开始训练）便可以开始训练
- 你可以在 `cmd`（命令行，即后台的黑色窗口）内看到训练进度

**特别注意：** 在Windows中，如果你不小心单击了该窗口的内部，则会 **暂停训练** ，请再次点击窗口且敲击键盘的`Enter`键来继续训练

## Step 3 模型的选择

在得到一堆模型后，可以选择一个合适的模型作为最终的模型

**特别注意：** 所有的数据参考都是虚的，请以自己的耳朵为最重要的尺标

### 选择方法

1. 直接选择最后一步的模型

2. 点击 `Tensorboardを開く`（打开 Tensorboard），参考各项数据的数值来进行选取（如loss/g/mel或者loss/g/total），或者参考 `eval` （如果有）下面的合成好的音频，自己听一下

3. 运行 `speech_mos.py` 来获得 mos 评分来衡量

> [!TIP]
> Tensorboard下数据的具体含义可以询问ai得到解释，问法：style-bert-vits2训练中的……是什么意思

## 常见问题

### Q：我的音频格式并非 wav，我该如何修改格式？

**A：** 推荐使用开源软件 [FFmpegFreeUI](https://github.com/Lake1059/FFmpegFreeUI)

### Q：我只有音频数据，在【🎹 音频数据】并没有现成的文本内容来做成【🖊 文本数据】的情况下，该如何快速制作【🖊 文本数据】？

**A：** 推荐使用 ASR 工具，可以尝试使用开源软件 [FasterWhisperGUI](https://github.com/CheshireCC/faster-whisper-GUI) ，模型可自行选择。
其中 [kotoba-whisper-v2.1-faster](https://hf-mirror.com/CheshireCC/kotoba-whisper-v2.1-faster) 是一个日语模型，经测试可在[FasterWhisperGUI](https://github.com/CheshireCC/faster-whisper-GUI)运行，这里只作参考，并非基于性能的推荐

> [!TIP]
> 如果您相对需要适合动漫语音的Whisper模型，您可以尝试 [anime-whisper](https://hf-mirror.com/litagin/anime-whisper)
> 如果您是Windows环境且为英伟达显卡，也可以尝试使用 [anime-whisper一键包](https://www.modelscope.cn/models/lingchat-research-studio/anime-whisper-micromamba/files) 来实现快速部署

### Q：我有大量的音频数据，分类筛选起来很麻烦，有没有什么工具助力我筛选

**A：** 我目前不了解有无自动化工具实现非人工筛选语音数据，如果你想要更快的人工筛选，推荐使用开源软件 [Audio Dataset Screener](https://github.com/2DIPW/audio_dataset_screener)

### Q：我该如何运行 speech_mos.py？

**A：**

1. 确保你安装了 python 或者下载了便携版的 python 并添加到了系统变量下
2. 在 Style-Bert-VITS2 项目文件夹下按住`Shift`键，并单击鼠标左键，在菜单中选择`在此处打开命令窗口`
3. 输入`.\venv\Scripts\activate`并运行
4. 输入`python speech_mos.py -m <你的模型名称>`并运行，例如：

```bash
python speech_mos.py -m Murasame
```

**注意事项：**

- 程序会开始下载评测模型到`C:\Users\Administrator`（以你电脑用户名为准）`\.cache`下
- 如果不想这样，请查询如何自定义设置 `HF_HOME` 来改变模型下载位置
- 如果无法下载模型，请搜索 `hf-mirror` ，按照此网站设置环境变量

**运行结束后：**

- **请不要关闭黑色窗口**，因为黑色窗口内会列出分数的排行榜，但不会写入任何文件来保存，因此需要先复制粘贴到自建文件，如此手动保存下来，以方便查阅
- 生成的其他数据将会保存在 Style-Bert-VITS2 项目下的`mos_results`文件夹

---

本训练教程作者: 123 [B 站主页](https://space.bilibili.com/387386808)
