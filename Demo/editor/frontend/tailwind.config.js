/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'gemini-bg': '#050505',      // 极深黑背景
        'gemini-panel': '#0e0e10',   // 面板黑
        'gemini-border': '#2a2a2a',  // 边框灰
        'gemini-orange': '#ff9900',  // 核心高亮橙
        'gemini-dim': '#666666',     // 暗文
        'gemini-blue': '#00bcd4',    // 分支流向色
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', 'monospace'], // 终端字体
      },
      boxShadow: {
        'glow': '0 0 15px rgba(255, 153, 0, 0.2)', // 橙色光晕
      },
      backgroundImage: {
        'grid-dots': 'radial-gradient(#333 1px, transparent 1px)', // 点阵背景
      }
    },
  },
  plugins: [],
}