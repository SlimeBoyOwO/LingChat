/**
 * 信纸专用 Markdown 渲染：在 MarkdownText 的安全规则（原始 HTML 一律转义）之上，
 * 增加 KaTeX 公式、highlight.js 代码着色和 SVG 绘图。
 *
 * 用独立的 Marked 实例，不影响聊天等其他 Markdown 渲染。SVG 以 data URI 图片显示，
 * 图片上下文里脚本和事件属性都不会执行。
 */
import { Marked, type TokenizerAndRendererExtension } from "marked";
import katex from "katex";
import hljs from "highlight.js/lib/common";
import "katex/dist/katex.min.css";
import "highlight.js/styles/github.css";
import { escapeHtml } from "@/utils/escapeHtml";

function renderTex(tex: string, displayMode: boolean): string {
  return katex.renderToString(tex.trim(), { displayMode, throwOnError: false });
}

function svgImage(svg: string): string {
  let source = svg.trim();
  // 作为图片加载的 SVG 必须带命名空间，模型常常省略
  if (!/<svg\b[^>]*\bxmlns=/.test(source)) {
    source = source.replace(/<svg\b/, '<svg xmlns="http://www.w3.org/2000/svg"');
  }
  return `<div class="letter-svg"><img alt="svg" src="data:image/svg+xml;charset=utf-8,${encodeURIComponent(source)}"></div>\n`;
}

const looksLikeSvg = (text: string) => /^\s*<svg\b[\s\S]*<\/svg>\s*$/.test(text);

const blockMath: TokenizerAndRendererExtension = {
  name: "blockMath",
  level: "block",
  start: (src) => src.match(/\$\$|\\\[/)?.index,
  tokenizer(src) {
    const match =
      /^\$\$([\s\S]+?)\$\$[^\S\n]*(?:\n|$)/.exec(src) ??
      /^\\\[([\s\S]+?)\\\][^\S\n]*(?:\n|$)/.exec(src);
    if (match) return { type: "blockMath", raw: match[0], text: match[1] };
  },
  renderer: (token) => `<div class="letter-math">${renderTex(token.text, true)}</div>\n`,
};

const inlineMath: TokenizerAndRendererExtension = {
  name: "inlineMath",
  level: "inline",
  start: (src) => src.match(/\$|\\\(/)?.index,
  tokenizer(src) {
    const display = /^\$\$([^\n]+?)\$\$/.exec(src);
    if (display) return { type: "inlineMath", raw: display[0], text: display[1], display: true };
    // $ 后紧跟空白或闭合 $ 后紧跟数字时不当公式，避免把「5$ 到 10$」这类金额吃掉
    const match = /^\$(?!\s)((?:\\.|[^\\$\n])+?)\$(?!\d)/.exec(src) ?? /^\\\((.+?)\\\)/.exec(src);
    if (match) return { type: "inlineMath", raw: match[0], text: match[1], display: false };
  },
  renderer: (token) => renderTex(token.text, token.display),
};

const letterMarked = new Marked({
  gfm: true,
  breaks: true,
  extensions: [blockMath, inlineMath],
  renderer: {
    html({ text }) {
      return looksLikeSvg(text) ? svgImage(text) : escapeHtml(text);
    },
    code({ text, lang }) {
      const language = (lang ?? "").trim().split(/\s+/)[0]?.toLowerCase() ?? "";
      if (language === "svg" || (["xml", "html"].includes(language) && looksLikeSvg(text))) {
        return svgImage(text);
      }
      if (language === "math" || language === "latex" || language === "tex") {
        return `<div class="letter-math">${renderTex(text, true)}</div>\n`;
      }
      const body = hljs.getLanguage(language)
        ? hljs.highlight(text, { language, ignoreIllegals: true }).value
        : escapeHtml(text);
      return `<pre><code class="hljs">${body}</code></pre>\n`;
    },
  },
});

export function renderLetterMarkdown(content: string): string {
  if (!content) return "";
  return (letterMarked.parse(content, { async: false }) as string) || "";
}
