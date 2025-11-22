# 🎨 Neo Series UI Design Specification
**核心理念**：Immersive Retro-Industrial（沉浸式复古工业风）。
**视觉隐喻**：界面不仅仅是 UI，而是一个正在运行的硬件终端。根据环境光照（日/夜），它在“高对比度 OLED 显示模式”与“褪色军用图纸模式”之间切换。

---

## 1. 核心色板 (Color Protocols)

系统基于 CSS 变量定义了两套完全独立的色彩协议。

### 🌑 Night Protocol (默认 / 暗色模式)
*视觉感受：黑暗房间中的老式琥珀色 CRT 显示器，高对比度，自发光。*

| 变量名 | 色值 (HEX) | 描述 |
| :--- | :--- | :--- |
| `--bg-primary` | `#080808` | **极夜黑**。非纯黑，带有微弱的噪点感。 |
| `--accent-main` | `#FF9900` | **琥珀橙**。核心交互色，高亮数据与边框。 |
| `--accent-sub` | `#00CCFF` | **全息蓝**。辅助信息，代表“冷”数据。 |
| `--text-main` | `#E0E0E0` | **荧光灰**。类似低亮度的屏幕文字。 |
| `--grid-line` | `rgba(255, 153, 0, 0.08)` | **暗网格**。极低透明度的背景参考线。 |
| `--vignette` | `#000000` | 四周压暗，模拟屏幕边缘光衰减。 |

### ☀️ Day Protocol (日间 / 亮色模式)
*视觉感受：一份放置已久的工业设计图纸，纸张泛黄，墨迹深沉，带有锈迹斑斑的标记。*

| 变量名 | 色值 (HEX) | 描述 |
| :--- | :--- | :--- |
| `--bg-primary` | `#E8E4D9` | **旧纸白**。暖色调，低饱和度，模拟骨纸质感。 |
| `--accent-main` | `#CC3300` | **工业锈**。取代明亮的橙色，呈现氧化的朱红色。 |
| `--accent-sub` | `#006D77` | **铜绿青**。深沉的蓝绿色，用于强调。 |
| `--text-main` | `#2B2B2B` | **重铅灰**。高对比度的墨色，保证可读性。 |
| `--grid-line` | `rgba(60, 50, 40, 0.08)` | **铅笔痕**。深褐色的背景网格。 |
| `--vignette` | `#595545` | **油污褐**。模拟纸张边缘的陈旧感。 |

---

## 2. 排版系统 (Typography)

字体选择强调“数据感”与“机械感”。

*   **标题 / Logo / 强调数字**: `Rajdhani` (Google Fonts)
    *   *特征*：方形结构，紧凑，带有明显的工业切割感。
    *   *字重*：Bold (700) / Medium (500)。
*   **装饰性大标题**: `Orbitron` (Google Fonts)
    *   *特征*：宽体，未来主义，用于极大的背景文字或章节头。
*   **正文 / 终端代码**: `Share Tech Mono` 或 `JetBrains Mono`
    *   *特征*：等宽字体，高辨识度，模拟打字机或控制台输出。

---

## 3. 组件构造 (Component Architecture)

UI 元素拒绝圆角，拥抱直角与线条。

*   **容器 (Cards/Windows)**:
    *   `border-radius: 0px` (绝对直角)。
    *   **1px 实线边框**。
    *   **角落锚点**：利用 `::before` 和 `::after` 在容器的对角（左上/右下）添加 8px-10px 的 L 形加粗边框或实心方块装饰。
*   **背景纹理**:
    *   必须包含全局的 `40px * 40px` 虚线或实线网格背景。
    *   卡片背景带有轻微的 `backdrop-filter: blur(2px)`，并非完全不透明。
*   **交互反馈**:
    *   Hover 时不使用渐变过渡，而是使用“闪烁”或“反色”的硬切换。
    *   按钮 Hover 时产生“扫描线掠过”的动画效果。

---

## 4. 视觉特效层 (Visual FX Layer)

这是灵魂所在，赋予静态页面以生命。

*   **可开关的CRT 扫描线 (Scanlines)**:
    *   覆盖全屏的横向线条纹理。
    *   夜间模式为黑色半透明；日间模式为暖棕色半透明（模拟纸张纹理干扰）。
*   **色差 (Chromatic Aberration)**:
    *   文字开启 `text-shadow`，向左红偏移 1px，向右蓝偏移 1px。日间模式下减弱偏移量。
*   **信号抖动 (Glitch)**:
    *   关键文本（如系统状态）应用随机透明度闪烁 (`opacity: 0.8 -> 1.0`)。
*   **光晕 (Glow)**:
    *   夜间模式下，文字和边框带有微弱的同色 `box-shadow` 或 `text-shadow` 辉光。日间模式移除辉光，改为锐利的阴影。

可开关的扫描线不仅仅是一张静态的背景图，它是一个**全屏覆层 (Overlay)**，模拟显示设备（或投影设备）的刷新机制。它必须具备**持续的纵向流动**和**微弱的随机闪烁**。

#### 技术实现原理
*   **DOM 结构**: 使用一个 `fixed` 定位的 `div` 或 `body::after` 伪元素，层级 `z-index: 999`，且必须设置 `pointer-events: none` 以穿透点击。
*   **核心纹理**: 使用 CSS `linear-gradient` 生成高频重复的横向线条。
*   **动态核心**: 利用 `background-position` 的 Y 轴变化实现无限滚动的“刷新”错觉。

#### 协议 A：夜间模式 (Night Protocol) - "Phosphor Gap"
*视觉隐喻：阴极射线管（CRT）显示器中，电子束扫描留下的荧光粉间隙。*

*   **纹理密度**: 高密度，每 4px 循环一次（2px 扫描线，2px 完全透明间隙）。
*   **色彩逻辑**: 扫描线使用**纯黑** (`#000000`) 的 Alpha 通道，间隙必须是 `transparent`。
*   **动态行为**:
    *   **流动**: 像旧电视一样，扫描线以恒定速度缓慢**向下**滚动。
    *   **强度**: 扫描线本身保持在 30% 的不透明度，间隙完全透明不影响整体亮度。
    *   **⚠️ 关键**: 夜间模式背景本身就暗，扫描线不透明度不能过高（建议 0.2-0.3），否则会让画面过暗难以阅读。
*   **代码参考**:
    ```css
    .scanlines {
      background: linear-gradient(
        to bottom,
        rgba(0, 0, 0, 0) 50%,      /* 仍带有黑色基调 */
        rgba(0,0,0,0.3) 50%
      );
      background-size: 100% 4px;
      animation: scanlineScroll 10s linear infinite;
    }
    ```

#### 协议 B：日间模式 (Day Protocol) - "Analog Interference"
*视觉隐喻：老式微缩胶片阅读器、投影仪的灯光频闪，或是旧印刷机留下的光栅纹理。*

*   **纹理密度**: 保持 4px 循环，但边缘羽化更柔和。
*   **色彩逻辑**: 关键区别点。不能用黑色，必须使用**暖焦茶色 / 铁锈棕** (`rgba(139, 69, 19, 0.15)`)。这能让画面看起来像是在透过一层老化的亚克力板或泛黄的描图纸观看。
*   **动态行为**:
    *   **流动**: 速度依旧，但视觉上更像是一种“材质的游移”而非电子扫描。
    *   **混合模式**: 建议配合 `mix-blend-mode: multiply` (正片叠底)，使扫描线像是印在纸上的痕迹。
*   **代码参考**:
    ```css
    [data-theme="light"] .scanlines {
      background: linear-gradient(
        to bottom,
        rgba(255,255,255,0) 50%,
        rgba(139, 69, 19, 0.15) 50%
      );
      background-size: 100% 4px;
      mix-blend-mode: multiply;
    }
    ```

#### 全局动画物理 (Global Animation Physics)

为了达到自然的“失真感”，必须定义两组并行的动画：

1.  **滚动 (The Roll)**: 模拟垂直同步刷新。
    ```css
    @keyframes scanlineScroll {
      0% { background-position: 0 0; }
      100% { background-position: 0 100%; }
    }
    ```

2.  **频闪 (The Flicker)**: *这是日夜模式切换时增加“真实感”的秘诀。*
    不应是持续的剧烈闪烁，而是极其微弱的电压不稳模拟。
    ```css
    .crt-active {
      animation: crtFlicker 0.15s infinite;
    }
    
    @keyframes crtFlicker {
      0% { opacity: 0.97; }
      50% { opacity: 1.0; }
      100% { opacity: 0.98; }
    }
    ```
---

## 5. 品牌标识设计 (Brand Identity)

基于您的要求，软件标题采用 `Rajdhani` 字体，通过字重和颜色的节奏变化来体现层级。

**设计规范代码 (React/Tailwind 风格):**

*   **NEO**: 白色/深墨色，代表纯粹的基底。
*   **STUDIO/CHAT/HEART**: 主题色（橙/锈），代表核心模块。
*   **PRO**: 极小字号，宽字距，低对比度，代表专业版次。

```jsx
/**
 * Component: BrandLogo
 * Font: 'Rajdhani', sans-serif
 */
const BrandLogo = ({ module = "STUDIO" }) => {
  return (
    <div className="flex items-baseline select-none" style={{ fontFamily: "'Rajdhani', sans-serif" }}>
      {/* 前缀：高亮白(夜) 或 重墨色(日) */}
      <span className="text-4xl font-bold text-white dark:text-[#2B2B2B] transition-colors">
        NEO
      </span>
      
      {/* 模块名：主题主色 (琥珀橙 / 工业锈) */}
      <span className="text-4xl font-bold text-[#FF9900] data-[theme=light]:text-[#CC3300] ml-2 transition-colors">
        {module}
      </span>
      
      {/* 版本号：暗淡色，超宽字距 */}
      <span className="text-sm font-bold text-[#666] data-[theme=light]:text-[#5c5c50] ml-3 tracking-[0.3em] uppercase">
        PRO_V3.1
      </span>
    </div>
  );
};
```

### 实际渲染效果参考
*   **Night**: **NEO** <span style="color:#FF9900">**STUDIO**</span> <span style="color:#666; font-size:0.6em; letter-spacing:2px">PRO</span>
*   **Day**: **<span style="color:#2B2B2B">NEO</span>** <span style="color:#CC3300">**STUDIO**</span> <span style="color:#5c5c50; font-size:0.6em; letter-spacing:2px">PRO</span>