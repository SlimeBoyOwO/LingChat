# 暮色节拍美术生成提示词

背景和角色使用内置 image_gen 生成。主程序使用的文件位于 `src/assets/minigames/twilight/`。角色参考项目已有的钦灵 Q 版图；原始输出、去背景脚本和完整帧表保留在本次独立原型中，主程序只打包整理后的四张透明帧。

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
