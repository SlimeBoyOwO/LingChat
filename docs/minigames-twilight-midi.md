# 暮色节拍 MIDI 配乐来源

## 真っ黒フランドール S

本曲按提供的 `真っ黒フランドール S 修正版.mid` 制作，界面采用文件中的曲名，不标为项目原创曲。它是东方相关的 MIDI 改编谱；原曲及已有改编的权利归对应作者，转换与程序合成不改变这些归属。源文件未填写可确认的 MIDI 编曲者姓名，因此不推断署名。

- 源文件 SHA-256：`cd91433d9d14fd440591ac1f5296fae344c0a275e44550f5f51fcd26d7dee073`
- MIDI 内部标题：`あの真黒フランドール・Sをうp主が数十人で弾けるような楽譜にしてみた`
- MIDI 原版权文本：`Copyright (C) 2009 `
- Format 1、480 PPQ、28 轨，其中 26 轨有音符；共 21,190 个音符、17 个速度事件。没有乐器切换、弯音或控制器事件，按钢琴声部合成。
- MIDI 本体时长约 212.617 秒；游戏加入四拍准备和 1.8 秒尾声，总长约 215.560 秒。

`src/assets/minigames/twilight/flandre-score.json` 保存源文件的音符 tick、时值、音高、力度、轨道名、速度图和上述来源信息。它是自动转换的紧凑数据，不手工调整原曲音高或节奏。四键谱面由 `flandre.js` 从旋律声部生成，不影响完整配乐的声部数量。

## 重新转换

使用带 `mido` 的 Python 环境，在仓库根目录运行：

```sh
python scripts/import-twilight-midi.py "path/to/真っ黒フランドール S 修正版.mid" src/assets/minigames/twilight/flandre-score.json
```

转换工具使用同音 FIFO 配对 note-on/off，并保留 tempo map。工具遇到不同乐器、控制器或弯音会报错，避免静默丢失演奏信息。Python 与 mido 仅用于导入；播放器和安装后的主程序不依赖它们。原 MIDI 不需要放进运行目录。
