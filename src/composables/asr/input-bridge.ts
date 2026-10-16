import { runtime, type AsrInputBridge } from "./state";

/** GameDialog 调用：注册输入框读写桥（partial 写入 / 拼接基准）。
 *  返回注销函数：组件卸载时调用，避免桥指向已卸载组件（路由切换间隙
 *  partial 写入死 ref——静默无效但有语义脏点） */
export function registerAsrInputBridge(b: AsrInputBridge): () => void {
  runtime.inputBridge = b;
  return () => {
    if (runtime.inputBridge === b) runtime.inputBridge = null;
  };
}

/** 流式 partial 写入输入框：整体替换语音追加块，不触碰 baseText 之前的内容
 * （ensureInit 的 stream_partial 监听调用；写入条件判定在调用方） */
export function writePartial(text: string): void {
  runtime.inputBridge?.setText(runtime.baseText + text);
}
