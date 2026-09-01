import { runtime } from "./state";

/** GameDialog 调用：注册输入框读写桥（partial 写入 / 拼接基准） */
export function registerAsrInputBridge(b: {
  getText: () => string;
  setText: (v: string) => void;
}): void {
  runtime.inputBridge = b;
}

/** 流式 partial 写入输入框：整体替换语音追加块，不触碰 baseText 之前的内容
 * （ensureInit 的 stream_partial 监听调用；写入条件判定在调用方） */
export function writePartial(text: string): void {
  runtime.inputBridge?.setText(runtime.baseText + text);
}
