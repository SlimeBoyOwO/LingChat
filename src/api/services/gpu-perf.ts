import { invoke } from '@tauri-apps/api/core'
import type { PerfTier } from './cpu-perf'
import { getTierLabel, getPerfTierColor, getSuggestedMaxFps } from './cpu-perf'

/** GPU 信息接口 */
export interface GpuInfo {
  /** 最高性能 GPU 的名称 */
  name: string
  /** 该 GPU 的性能等级 */
  tier: PerfTier
  /** 当前平台是否支持 GPU 分级（Android / ARM macOS 不支持） */
  is_applicable: boolean
  /** 不适用 / 未检测到 GPU 时的友好提示（仅在 message 有值时显示） */
  message: string | null
}

/** localStorage 键名 */
const STORAGE_KEY = 'lingchat-gpu-perf'

/** 从 localStorage 读取缓存的 GPU 信息 */
function loadFromCache(): GpuInfo | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    return JSON.parse(raw) as GpuInfo
  } catch {
    return null
  }
}

/** 将 GPU 信息写入 localStorage */
function saveToCache(info: GpuInfo): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(info))
  } catch {
    // localStorage 不可用时静默失败
  }
}

/** 清除 localStorage 缓存 */
function clearCache(): void {
  try {
    localStorage.removeItem(STORAGE_KEY)
  } catch {
    // 静默失败
  }
}

/**
 * 获取 GPU 信息（优先使用 localStorage 缓存）
 *
 * 首次调用时调用 Tauri 后端检测，结果存入 localStorage；
 * 后续启动直接从 localStorage 读取，不再调用后端。
 */
export async function getGpuInfo(): Promise<GpuInfo> {
  // 优先读取 localStorage 缓存
  const cached = loadFromCache()
  if (cached) {
    return cached
  }

  // 缓存不存在，调后端检测
  const info = await invoke<GpuInfo>('get_gpu_info')
  saveToCache(info)
  return info
}

/**
 * 重新检测 GPU 性能（清除 localStorage 缓存后重新检测）
 */
export async function redetectGpu(): Promise<GpuInfo> {
  clearCache()
  const info = await invoke<GpuInfo>('redetect_gpu')
  saveToCache(info)
  return info
}

/**
 * 读取 WebGL 实际渲染器字符串（反映 WebView2/Chromium 实际合成所用的 GPU）。
 *
 * 通过 `WEBGL_debug_renderer_info.UNMASKED_RENDERER_WEBGL` 拿到如
 * "ANGLE (NVIDIA, NVIDIA GeForce RTX 4060 Laptop GPU Direct3D11 ... , D3D11)" 的字符串。
 * 该值就是程序当前正在调用的 GPU。读取失败返回 null。
 */
function readWebGlRenderer(): string | null {
  try {
    const canvas = document.createElement('canvas')
    const gl =
      canvas.getContext('webgl') ||
      (canvas.getContext('experimental-webgl') as WebGLRenderingContext | null)
    if (!gl) return null
    const debug = gl.getExtension('WEBGL_debug_renderer_info')
    const renderer = debug
      ? (gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) as string)
      : (gl.getParameter(gl.RENDERER) as string)
    return renderer ? String(renderer).trim() : null
  } catch {
    return null
  }
}

/**
 * 获取「当前实际调用的 GPU」（通过 WebGL 渲染器字符串，交由后端 grade() 定级）。
 *
 * 与 getGpuInfo() 的「本机最高性能 GPU」不同：
 * - getGpuInfo(): 硬件枚举后取最高（用于综合分级 / 自动画质配置）
 * - getActiveGpu(): 反映程序当前真正在用的那张卡（设置页展示用）
 */
export async function getActiveGpu(): Promise<GpuInfo> {
  const renderer = readWebGlRenderer()
  if (!renderer) {
    return {
      name: '',
      tier: 'Low',
      is_applicable: false,
      message: '无法读取 WebGL 渲染器',
    }
  }
  return await invoke<GpuInfo>('grade_active_gpu', { renderer })
}

export { getTierLabel, getPerfTierColor, getSuggestedMaxFps }
export type { PerfTier }
