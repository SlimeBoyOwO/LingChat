/**
 * TODO 小组件独立窗口：开合、置顶与窗口几何的持久化。
 *
 * 窗口由前端创建而不是 Rust：置顶状态与窗口几何都存在 localStorage，Rust 侧读不到，
 * 只有前端创建才能把存档值一次性传进窗口构造参数。
 *
 * 「启动自动打开」与「侧边栏开关」两条路径必须构造出完全一致的窗口，故参数收在这里，
 * 不各抄一份。
 */
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { availableMonitors, getCurrentWindow } from "@tauri-apps/api/window";

export const TODO_WIDGET_LABEL = "todo_widget";
/** 小组件是否应处于显示状态（跨启动记忆） */
export const TODO_WIDGET_OPEN_KEY = "lingchat_todo_widget_open";
/** 是否始终置顶（跨启动记忆） */
export const TODO_WIDGET_ON_TOP_KEY = "lingchat_todo_widget_on_top";
/** 窗口几何，逻辑像素（跨启动记忆） */
export const TODO_WIDGET_GEOMETRY_KEY = "lingchat_todo_widget_geometry";
/** 小组件开合状态变化，由小组件窗发往主窗口 */
export const TODO_WIDGET_STATE_EVENT = "todo-widget:state";

const DEFAULT_WIDTH = 320;
const DEFAULT_HEIGHT = 420;
const MIN_WIDTH = 240;
const MIN_HEIGHT = 220;
/** 拖动/缩放期间事件很密集，防抖后再落盘 */
const GEOMETRY_SAVE_DEBOUNCE_MS = 400;
/** 判定「还拖得回来」的顶部条带高度 */
const REACHABLE_STRIP_PX = 32;

export interface TodoWidgetGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

function readRaw(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeRaw(key: string, value: string | null): void {
  try {
    if (value === null) {
      localStorage.removeItem(key);
    } else {
      localStorage.setItem(key, value);
    }
  } catch {
    // localStorage 不可用时静默忽略，退化成「本次会话有效」
  }
}

/** 用户是否把小组件设定为显示。要判断窗口此刻是否真开着，用 isTodoWidgetOpen */
export function isTodoWidgetEnabled(): boolean {
  return readRaw(TODO_WIDGET_OPEN_KEY) === "1";
}

/** 记录「小组件应处于显示状态」的意图（跨启动记忆） */
export function setTodoWidgetEnabled(enabled: boolean): void {
  writeRaw(TODO_WIDGET_OPEN_KEY, enabled ? "1" : null);
}

/** 默认置顶：小组件的常态就是浮在别的窗口之上 */
export function isTodoWidgetOnTop(): boolean {
  return readRaw(TODO_WIDGET_ON_TOP_KEY) !== "0";
}

export function setTodoWidgetOnTop(onTop: boolean): void {
  writeRaw(TODO_WIDGET_ON_TOP_KEY, onTop ? "1" : "0");
}

function readGeometry(): TodoWidgetGeometry | null {
  const raw = readRaw(TODO_WIDGET_GEOMETRY_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<TodoWidgetGeometry>;
    const { x, y, width, height } = parsed;
    if ([x, y, width, height].some((v) => typeof v !== "number" || !Number.isFinite(v))) {
      return null;
    }
    return {
      x: x as number,
      y: y as number,
      width: Math.max(MIN_WIDTH, width as number),
      height: Math.max(MIN_HEIGHT, height as number),
    };
  } catch {
    return null;
  }
}

export function saveTodoWidgetGeometry(geometry: TodoWidgetGeometry): void {
  writeRaw(TODO_WIDGET_GEOMETRY_KEY, JSON.stringify(geometry));
}

/**
 * 存档位置是否仍落在某个显示器的工作区内。
 *
 * 外接屏被拔掉后，存档坐标会指向一块已经不存在的屏幕；而小组件不占任务栏，
 * 跑到屏幕外就既看不见也几乎找不回来，所以这种情况下宁可放弃位置、退回居中。
 */
async function isPositionReachable(geometry: TodoWidgetGeometry): Promise<boolean> {
  try {
    const monitors = await availableMonitors();
    if (monitors.length === 0) return true;
    return monitors.some((monitor) => {
      // 显示器信息是物理像素，存档是逻辑像素
      const scale = monitor.scaleFactor || 1;
      const left = monitor.workArea.position.x / scale;
      const top = monitor.workArea.position.y / scale;
      const right = left + monitor.workArea.size.width / scale;
      const bottom = top + monitor.workArea.size.height / scale;
      return (
        geometry.x < right &&
        geometry.x + geometry.width > left &&
        geometry.y + REACHABLE_STRIP_PX > top &&
        geometry.y < bottom
      );
    });
  } catch {
    // 查不到显示器信息时不要拦，让用户自己拖
    return true;
  }
}

/** 打开小组件窗口；已存在则聚焦。重复调用安全。 */
export async function openTodoWidget(): Promise<void> {
  setTodoWidgetEnabled(true);

  const existing = await WebviewWindow.getByLabel(TODO_WIDGET_LABEL);
  if (existing) {
    await existing.unminimize().catch(() => {});
    await existing.setFocus().catch(() => {});
    return;
  }

  const saved = readGeometry();
  const reachable = saved ? await isPositionReachable(saved) : false;
  const placement =
    saved && reachable
      ? { x: saved.x, y: saved.y, width: saved.width, height: saved.height }
      : { width: saved?.width ?? DEFAULT_WIDTH, height: saved?.height ?? DEFAULT_HEIGHT };

  const widget = new WebviewWindow(TODO_WIDGET_LABEL, {
    // ?window= 是必需的：main.ts 靠它决定不注册全局事件管线
    url: "index.html?window=todo_widget",
    title: "LingChat TODO",
    ...placement,
    minWidth: MIN_WIDTH,
    minHeight: MIN_HEIGHT,
    resizable: true,
    decorations: false,
    transparent: false,
    shadow: false,
    skipTaskbar: true,
    alwaysOnTop: isTodoWidgetOnTop(),
    focus: true,
  });
  widget.once("tauri://error", (e) => console.error("[TodoWidget] 创建窗口失败:", e));
}

/**
 * 关闭小组件窗口。
 * 清「应显示」记忆与通知主窗口由 shutdownTodoWidget 负责 —— 窗口的 close-requested
 * 是唯一的收口，这样 Alt+F4 之类的系统关闭路径也能同步到侧边栏开关。
 */
export async function closeTodoWidget(): Promise<void> {
  try {
    const existing = await WebviewWindow.getByLabel(TODO_WIDGET_LABEL);
    if (existing) await existing.close();
  } catch (e) {
    console.error("[TodoWidget] 关闭窗口失败:", e);
  }
}

/**
 * 由小组件窗口在收到关闭请求时调用：清掉「应显示」记忆、通知主窗口，然后销毁自己。
 *
 * 调用方必须先 preventDefault —— 否则窗口会在广播发出去之前就被拆掉，主窗口收不到。
 */
export async function shutdownTodoWidget(): Promise<void> {
  setTodoWidgetEnabled(false);
  const win = getCurrentWindow();
  try {
    await win.emitTo("main", TODO_WIDGET_STATE_EVENT, false);
  } catch (e) {
    console.warn("[TodoWidget] 通知主窗口失败:", e);
  }
  try {
    await win.destroy();
  } catch (e) {
    console.error("[TodoWidget] 销毁窗口失败:", e);
  }
}

/** 小组件窗口此刻是否真的开着 */
export async function isTodoWidgetOpen(): Promise<boolean> {
  try {
    return Boolean(await WebviewWindow.getByLabel(TODO_WIDGET_LABEL));
  } catch {
    return false;
  }
}

/**
 * 由小组件窗口自身调用：监听自身的缩放/移动并落盘逻辑坐标。
 * 返回取消监听的清理函数。
 */
export async function trackTodoWidgetGeometry(): Promise<() => void> {
  const win = getCurrentWindow();
  let timer: ReturnType<typeof setTimeout> | null = null;

  const flush = async () => {
    timer = null;
    try {
      const [position, size, scale] = await Promise.all([
        win.outerPosition(),
        win.innerSize(),
        win.scaleFactor(),
      ]);
      const factor = scale || 1;
      saveTodoWidgetGeometry({
        x: position.x / factor,
        y: position.y / factor,
        width: size.width / factor,
        height: size.height / factor,
      });
    } catch (e) {
      console.warn("[TodoWidget] 记录窗口几何失败:", e);
    }
  };

  const schedule = () => {
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => void flush(), GEOMETRY_SAVE_DEBOUNCE_MS);
  };

  const unlisten = await Promise.all([win.onResized(schedule), win.onMoved(schedule)]);
  return () => {
    if (timer !== null) clearTimeout(timer);
    unlisten.forEach((fn) => fn());
  };
}
