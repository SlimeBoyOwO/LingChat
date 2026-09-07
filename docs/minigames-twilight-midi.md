# 暮色节拍：U.N. Owen 配乐来源

## 来源与署名

曲目为《U.N.オーエンは彼女なのか？》（U.N. Owen Was Her?），是东方Project二次创作。原曲作曲者为 ZUN / 上海アリス幻樂団，参考钢琴谱为 **DMBN / 東方ピアノEasyモード** 发布的 **Normal** 版本。

- [曲目目录](https://easypianoscore.jp/sheetList.php?titleid=kouma)
- [Normal 乐谱与 MIDI 下载](https://easypianoscore.jp/download.php?musicName=un&musicLevel=normal&ext=zip&inst=)
- [谱源的使用说明](https://easypianoscore.jp/kenri.html)
- [东方Project二次创作指引](https://touhou-project.news/guideline/)

DMBN 的使用说明允许注明来源后的非营利用途使用、修改和分发；此谱源有独立的使用条件，不随程序代码改为 GPL 或其他通用素材许可。商业用途及视频收益化应按谱源说明另行处理。游戏选曲界面和本文件均显示原作者、钢琴谱作者及来源，作品以二次创作标示。

源文件名为 U.N. Owen was her？\_Normal.mid，SHA-256 为：
082dc2e0465a0f8102597b6b2bcd7271f4e20c86530e8126fc8b36f613382b13

该文件为 Format 1、960 PPQ、两条钢琴声部，总计 1,317 个音符。主体 150 BPM，包含 10 个速度事件，以及 5/4、4/4、2/4 拍号变化。参考文件自带的开头空白被移除，游戏统一添加四拍准备和 1.8 秒尾声，总长约 133.215 秒。

## 游戏中的编配

- 右手每次起音的最高音作为主旋律，共 574 个；保留该普通谱的音级、节奏及连断关系，整体上移 **12 个半音（一个八度）**。
- 右手其余 197 个音符作为内声部和声，下移一个八度，保持原谱的和声关系。左手的 546 个低音音符保留原音区。
- 旋律、和声、低音分别设置音色、包络和音量；音区调整保持八度等价，A4=440 Hz，不增加失谐叠音。原谱的延音踏板以合成器有限的释放尾音代替，让高音旋律的快速起音保持清楚。
- 谱面由主旋律及小节强拍的低音重音生成，共 **650 音符、76 组双押、13 个长按**。根据实际拍号确定小节起点，保留开头 5/4 和中间 2/4 的强拍位置。每组最多双押，同轨释放后至少间隔 170 ms。
- 原谱的变速同步影响音乐、判定时间、HUD、节拍灯光与暂停恢复准备拍。深红场景和实际音频波形继续使用。

打包数据位于 src/assets/minigames/twilight/un-owen-score.json，包含上述三个声部、速度、拍号与来源信息。它取代此前的黑乐谱整理数据，播放时无需联网或查找本机 MIDI。

## 重新导入

使用带 mido 的 Python 环境，在仓库根目录运行：

```sh
python scripts/import-twilight-midi.py "path/to/un-normal.mid" src/assets/minigames/twilight/un-owen-score.json
```

工具配对 note-on/off，按右手旋律/和声和左手低音拆分，并执行固定的八度调整；不重新推断和弦、不套用黑乐谱抽取规则。遇到预期以外的轨道、控制器或乐器事件会报错。Python 和 mido 仅用于离线导入，运行版不依赖它们。
