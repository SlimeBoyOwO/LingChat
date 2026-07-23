import { DIALOG_MAX_BASE, DIALOG_MIN_BASE } from './constants'

const BOX_WIDTH_FRACTION = 0.85
const APP_WIDTH_BASE = 240
const PADDING_X_BASE = 18
const FONT_SIZE_BASE = 15
const LINE_HEIGHT_RATIO = 1.375
const TEXT_DEFAULT_LINES = 2

const CONTENT_WIDTH = BOX_WIDTH_FRACTION * APP_WIDTH_BASE - 2 * PADDING_X_BASE
const CHARS_PER_LINE_CJK = CONTENT_WIDTH / FONT_SIZE_BASE
const LINE_HEIGHT_PX = FONT_SIZE_BASE * LINE_HEIGHT_RATIO

function isCJK(ch: string): boolean {
  const c = ch.charCodeAt(0)
  return (
    (c >= 0x4e00 && c <= 0x9fff) ||
    (c >= 0x3400 && c <= 0x4dbf) ||
    (c >= 0xf900 && c <= 0xfaff)
  )
}

function isFullWidthPunctuation(ch: string): boolean {
  const c = ch.charCodeAt(0)
  return (
    (c >= 0x3000 && c <= 0x303f) ||
    (c >= 0xff01 && c <= 0xff60) ||
    (c >= 0xffe0 && c <= 0xffe6)
  )
}

function countVisualCharWidth(text: string): number {
  let width = 0
  for (const ch of text) {
    if (isCJK(ch) || isFullWidthPunctuation(ch)) {
      width += 1
    } else if (ch === '\n') {
      width += CHARS_PER_LINE_CJK
    } else {
      width += 0.5
    }
  }
  return Math.max(width, 1)
}

export function estimateDialogHeight(text: string): number {
  const visualWidth = countVisualCharWidth(text)
  const lines = Math.max(1, Math.ceil(visualWidth / CHARS_PER_LINE_CJK))
  const extraLines = Math.max(0, lines - TEXT_DEFAULT_LINES)
  const extraHeight = extraLines * LINE_HEIGHT_PX

  const dialogH = Math.round(
    Math.min(Math.max(DIALOG_MIN_BASE + extraHeight, DIALOG_MIN_BASE), DIALOG_MAX_BASE),
  )
  return dialogH
}
