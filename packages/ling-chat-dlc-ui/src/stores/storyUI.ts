import { useSettingsStore } from "@lingchat/host/stores/modules/settings";
import { HORROR_EFFECT_KEYS } from "@lingchat/host/components/game/standard/particles";
export interface StoryUIState {
  /** 每次音效事件递增，确保相同路径也触发重播。 */
  soundEffectSeq: number;
  /** 角色语音（TTS）播放倍率，由剧本 voice_shift 事件设置；<1 降调=恶魔音，1.0 正常 */
  voiceRate: number;
  /** 角色语音纯音调偏移（半音数，负数=低沉），由剧本 voice_shift 事件设置；0 正常。
   *  经 Web Audio detune 实现，不改变语速，可与 voiceRate 叠加 */
  voicePitch: number;
  /** 进入剧本前的自由对话 BGM（会话级，退出剧本时恢复；null = 不在剧本中） */
  preScriptBgm: string | null;
  /** 进入剧本前的 BGM 循环模式（同上，随 preScriptBgm 一起恢复） */
  preScriptBgmMode: "loop-list" | "loop-single" | "random" | null;
  /** 进入剧本前的自由对话背景图（剧本换的背景不得带出剧本外） */
  preScriptBackground: string | null;
  /** true 时 BGM 状态变化不写入 session（剧本运行期间屏蔽，防剧本 BGM 泄漏到下次启动） */
  bgmPersistBlocked: boolean;
  /** 突脸惊吓：图片路径（空串 = 无演出） */
  jumpscareImage: string;
  /** 突脸音效路径（由 Jumpscare 组件自行播放） */
  jumpscareSound: string;
  /** 突脸收场时间戳（ms, Date.now() 基准），到点组件自行隐藏 */
  jumpscareUntil: number;

  /** 立绘闪现（DDLC 式崩坏一闪）：null = 无演出；seq 递增保证同情绪也能重复触发 */
  spriteFlash: {
    roleId: number;
    emotion: string;
    duration: number;
    seq: number;
  } | null;

  /** 立绘噪点侵蚀（DDLC n_rects_ghost 式）：null = 无演出；常驻到显式清除 */
  spriteNoise: {
    roleId: number;
    noise: string;
    fadeInSec: number;
    seq: number;
  } | null;

  /** 恐怖剧本入口过渡阶段：'' 无 / 'freeze' 卡死 / 'static' 花屏 */
  horrorEntryPhase: "" | "freeze" | "static";

  /** 删角色文件彩蛋（DDLC ghost menu 对应物）：.chr 被全删的剧本进入时
   *  锁成纯黑底 + 黑白幽灵立绘，不给任何文字和按钮出口；null = 无锁定 */
  ghostLock: { scriptName: string; assetDir: string } | null;
  /** 幽灵锁定中点窗口 X 的退出突脸（DDLC quit 放大脸）：true = 正在演出，随后退出 */
  ghostQuitZoom: boolean;

  /** BSOD 假异常窗口的 trace 行文本（剧本经 background_effect.text 自带；空串 = 用通用默认） */
  bsodText: string;
  /** BSOD 彩蛋独白（background_effect.echo；空串 = 不显示独白） */
  bsodEcho: string;

  /** DLC 变更信号：导入/卸载后 +1，主菜单等监听它刷新剧本列表与 DLC 提示 */
  dlcRefreshToken: number;
}
export const storyState = (): StoryUIState => ({
  soundEffectSeq: 0,
  voiceRate: 1,
  voicePitch: 0,
  preScriptBgm: null,
  preScriptBgmMode: null,
  preScriptBackground: null,
  bgmPersistBlocked: false,
  // 突脸惊吓演出初始状态
  jumpscareImage: "",
  jumpscareSound: "",
  jumpscareUntil: 0,

  // 立绘闪现演出初始状态
  spriteFlash: null,

  // 立绘噪点侵蚀演出初始状态
  spriteNoise: null,

  horrorEntryPhase: "",

  // 幽灵锁定（删角色文件彩蛋）初始状态
  ghostLock: null,
  ghostQuitZoom: false,

  // BSOD 假异常窗口的剧本自带文本（空串 = 通用默认/无独白）
  bsodText: "",
  bsodEcho: "",

  dlcRefreshToken: 0,
});
type StoryActionContext = StoryUIState & {
  currentSoundEffect: string;
  clearJumpscare(): void;
};
// 立绘闪现序号：每次触发递增，同情绪连闪也能触发组件 watch
let spriteFlashSeq = 0;
// 立绘噪点侵蚀序号：同上，保证同预设重复触发也能被组件观察
let spriteNoiseSeq = 0;

export const storyActions = {
  /** 触发一次短音效；序号让同一路径连续事件也可观察。 */
  triggerSoundEffect(this: StoryActionContext, path: string) {
    this.currentSoundEffect = path || "None";
    this.soundEffectSeq += 1;
  },
  /** 触发突脸惊吓：图片全屏闪现 durationSec 秒，自带音效 */
  triggerJumpscare(this: StoryActionContext, image: string, sound: string, durationSec: number) {
    this.jumpscareImage = image;
    this.jumpscareSound = sound;
    this.jumpscareUntil = Date.now() + Math.max(0.15, durationSec) * 1000;
  },
  /** 立即收场（组件卸载或剧本结束时兜底调用） */
  clearJumpscare(this: StoryActionContext) {
    this.jumpscareImage = "";
    this.jumpscareSound = "";
    this.jumpscareUntil = 0;
  },
  /** 打开幽灵锁定（.chr 被全删的剧本入口）：全屏纯黑底 + 黑白立绘，无任何 UI 出口 */
  openGhostLock(this: StoryActionContext, scriptName: string, assetDir: string) {
    this.ghostLock = { scriptName, assetDir };
  },
  /** 解除幽灵锁定（玩家放回 .chr 后轮询发现已解锁） */
  closeGhostLock(this: StoryActionContext) {
    this.ghostLock = null;
    this.ghostQuitZoom = false;
  },
  /** 幽灵锁定中点窗口 X：DDLC quit 式放大脸演出，演完由 App.vue 真正退出 */
  triggerGhostQuitZoom(this: StoryActionContext) {
    if (this.ghostLock) this.ghostQuitZoom = true;
  },
  /** 立绘闪现：把 roleId 的立绘短暂替换为 emotion 版本，duration 秒后由组件自动还原 */
  triggerSpriteFlash(
    this: StoryActionContext,
    roleId: number,
    emotion: string,
    durationSec: number,
  ) {
    spriteFlashSeq += 1;
    this.spriteFlash = {
      roleId,
      emotion,
      duration: Math.max(0.12, durationSec),
      seq: spriteFlashSeq,
    };
  },
  /**
   * 立绘噪点侵蚀（DDLC n_rects_ghost 式）：在 roleId 脸部挂上每帧随机抖动的
   * 黑色噪点团，fadeInSec 秒淡入后常驻；noise 传 'none'/空串 = 清除演出。
   */
  triggerSpriteNoise(this: StoryActionContext, roleId: number, noise: string, fadeInSec: number) {
    if (!noise || noise === "none" || noise === "None") {
      this.spriteNoise = null;
      return;
    }
    spriteNoiseSeq += 1;
    this.spriteNoise = {
      roleId,
      noise,
      fadeInSec: Math.max(0, fadeInSec),
      seq: spriteNoiseSeq,
    };
  },
  /**
   * 恐怖特效残留清理：当前特效包含恐怖向名称时重置为 'None'。
   * 剧本异常退出/中途返回/重启后都可能残留，在剧本结束、进入剧本、应用启动时调用。
   */
  resetHorrorEffects(this: StoryActionContext) {
    const current = useSettingsStore().display.backgroundEffect;
    if (HORROR_EFFECT_KEYS.some((effect) => current.split("+").includes(effect))) {
      useSettingsStore().setBackgroundEffect("None");
    }
    this.clearJumpscare();
    this.spriteFlash = null;
    this.spriteNoise = null;
    // 恶魔音残留一并清理（剧本结束/进入/启动时都会走到这里）
    this.voiceRate = 1;
    this.voicePitch = 0;
    // BSOD 的剧本自带文本一并清掉，回到通用默认
    this.bsodText = "";
    this.bsodEcho = "";
  },
  /**
   * 恐怖剧本入口过渡：卡死 1.1s → 花屏 0.8s，结束后 resolve。
   * 调用方 await 它再执行真正的进入逻辑。
   */
  beginHorrorEntry(this: StoryActionContext): Promise<void> {
    return new Promise((resolve) => {
      this.horrorEntryPhase = "freeze";
      setTimeout(() => {
        this.horrorEntryPhase = "static";
      }, 1100);
      setTimeout(() => {
        this.horrorEntryPhase = "";
        resolve();
      }, 1900);
    });
  },
};
