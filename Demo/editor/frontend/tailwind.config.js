/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  // 启用 class 策略以便手动控制 dark mode
  darkMode: 'class', 
  theme: {
    extend: {
      colors: {
        neo: {
          bg: 'var(--bg-primary)',
          panel: 'var(--bg-secondary)',
          border: 'var(--border-dim)',
          main: 'var(--accent-main)',
          sub: 'var(--accent-sub)',
          text: 'var(--text-main)',
          dim: 'var(--text-dim)',
        }
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', 'monospace'],
        display: ['"Rajdhani"', 'sans-serif'],
        deco: ['"Orbitron"', 'sans-serif'],
      },
      backgroundImage: {
        'grid-pattern': 'linear-gradient(var(--grid-line) 1px, transparent 1px), linear-gradient(90deg, var(--grid-line) 1px, transparent 1px)',
      },
      boxShadow: {
        'neo': 'var(--shadow-glow)',
      }
    },
  },
  plugins: [],
}