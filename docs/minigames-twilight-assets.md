# 暮色节拍美术生成提示词

背景和角色使用内置 image_gen 生成。主程序使用的文件位于 `src/assets/minigames/twilight/`。角色参考项目已有的钦灵 Q 版图；四张原有动作帧保留，`qinling-idle.png` 提供六帧待机动作。

## 背景提示词

```text
Use case: stylized-concept. Asset type: a production-ready 2D pixel-art background for an original side-view rhythm minigame in LingChat.
Create a richly composed, unmistakably pixel-art Japanese shrine courtyard at dusk, wide 16:9 landscape. Orange vermilion torii gate, a modest wooden shrine with dark tiled roofs, warm small lanterns, pink cherry trees, distant violet mountains and a mauve sunset sky. Fixed side-on game camera, layered scenery. The bottom quarter is a clear continuous stone courtyard platform so a small playable character can be placed there. Keep the lower-middle and right courtyard spacious for gameplay overlays.
Art direction: crisp deliberate 16-bit pixel clusters, restrained 40-color palette, tiny dithered shadows, blocky stair-step silhouettes, charming handcrafted game environment, strong readable architecture, medium detail, muted plum shadows and warm peach light. Render as a coherent low-resolution 640x360 pixel-art scene, upscaled with hard nearest-neighbor edges. This is an actual game background, not a screenshot or concept sheet.
No characters, no UI, no lettering, no logos, no watermarks, no border, no black bars, no video player, no collage, no blur, no photorealism, no 3D render. Fill the complete image. Original shrine design; no existing franchise emblems.
```

## 角色提示词

```text
Use case: stylized-concept. Asset type: production 2D pixel-art character sprite sheet for a small LingChat rhythm game.
Use the supplied image only as the character identity reference: Qinling, a cheerful white wolf-eared girl, long white hair, cyan eyes, teal oversized hoodie, small blue sneakers, fluffy white tail, blue sunglasses resting on her head. Reinterpret her as crisp 16-bit pixel art, approximately 64x80 logical pixels per full-body sprite, with hard square pixel clusters, compact 2.5-head-tall proportions, dark purple single-pixel outlines and restrained palette.
Output one square sprite sheet divided into an exact 2 by 2 invisible grid. Each quadrant contains exactly one full-body sprite, centered at exactly the same horizontal offset within its cell, same scale, and identical foot baseline near the bottom of its cell. Large empty padding around each sprite. Top-left: relaxed idle, both feet grounded, hands in front. Top-right: rhythm tap, left arm extended diagonally, one foot stepping. Bottom-left: rhythm tap, right arm extended diagonally, opposite foot stepping. Bottom-right: small happy success pose, hands raised, both feet at same grounded baseline. Keep the face, clothes, size and silhouette consistent.
Genuinely transparent RGBA background across the entire sheet outside the four characters, no painted checkerboard, no colored matte, no floor, no cast shadow, no frame, no grid lines, no labels, no text, no blur, no antialiasing. All four sprites completely within their own equal square cells and never touching another cell. This is a usable sprite sheet, not concept art.
```

## 音乐

《灯下回声》是本任务通过 music.js 编写的原创程序合成练习曲，112 BPM，A 小调，32 小节，前置四拍准备。音频导出脚本保留在原型中，未使用外部录音或采样。

## 待机动作提示词

内置 image_gen 编辑模式，以 `qinling-0.png` 为角色参考生成六帧；清理棋盘背景后保存为 1536×1024 RGBA 图集 `qinling-idle.png`。`idle.js` 记录各帧边界与脚底锚点，绘制时按相同尺度定位，避免头发高度差引起站位漂移。

```text
Use case: identity-preserve. Asset type: pixel-art idle animation sprite sheet for a LingChat minigame.
Input image 1 is the exact existing character identity and art style reference. Create ONE coherent six-frame idle animation sheet, exact 3 columns by 2 rows, row-major chronological frames. White wolf-eared girl Qinling, long white hair, cyan eyes, teal oversized hoodie, small blue sneakers, fluffy white tail on viewer right, blue sunglasses on head. Preserve the supplied face, outfit, head/body proportions, colors, pixel clusters and front-facing stance with both hands held together in front.
Frame 1 relaxed open eyes; frame 2 gentle inhalation with slightly lifted shoulders and subtly swaying tail; frame 3 peak gentle inhale, tiny ear twitch, eyes open; frame 4 half-closed eyelids; frame 5 fully closed eyes for a blink; frame 6 eyes open, shoulders relaxed, tail returning. Keep movement very subtle, no bouncing, no jumping, no foot movement or leg changes. All SIX frames must use IDENTICAL shoe pixels and IDENTICAL grounded foot coordinates within their equal cells. Identical head size and body scale. Only torso breathing, hair tips, tail, ears and eyelids animate. A usable game animation, not six redesigns.
Crisp low-resolution 16-bit pixel art with hard nearest-neighbor edges, no blur. Each character fully inside its equal cell with generous transparent padding. Absolutely genuine RGBA transparency outside the six sprites; no painted checkerboard, no gray squares, no white or black matte, no shadows or floor, no labels, no grid lines, no text, no borders. Frame 1 should match the reference especially closely.
```

生成帧负责眨眼、耳朵与尾巴的细小变化。呼吸另外使用 4.4 秒周期：1.6 秒吸气、2.4 秒呼气、0.4 秒休息，最大抬肩 2 个画布像素。只拉伸衣服所在的躯干区，头部随肩膀移动，腿部和脚底固定。最近邻采样保留像素边缘；暂停、后台、关闭节拍特效或系统减少动态效果时停止呼吸。
