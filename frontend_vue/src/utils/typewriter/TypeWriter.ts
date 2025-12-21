import { useUIStore } from "../../stores/modules/ui/ui";

export class TypeWriter {
  private element: HTMLInputElement | HTMLTextAreaElement;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private abortController: AbortController | null;
  private speed: number;
  private isFinished: boolean;
  private onFinishCallback: (() => void) | null;
  private audioContext: AudioContext | null;
  private soundBuffers: AudioBuffer[];
  private textBuffers: string;
  private readonly soundUrls: string[];

  constructor(element: HTMLInputElement | HTMLTextAreaElement) {
    this.element = element;
    this.timer = null;
    this.abortController = null;
    this.speed = 50; // 默认速度
    this.isFinished = false;
    this.onFinishCallback = null;
    this.audioContext = null;
    this.soundBuffers = [];
    this.textBuffers = "";
    this.soundUrls = ["../audio_effects/对话.wav"];
  }

  // 初始化音频系统
  private async initAudio(): Promise<void> {
    try {
      this.audioContext = new (window.AudioContext ||
        (window as any).webkitAudioContext)();
      await this.loadSounds();
    } catch (e) {
      console.warn("音频初始化失败:", e);
      const uiStore = useUIStore();
      uiStore.enableChatEffectSound = false;
    }
  }

  // 加载音效
  private async loadSounds(): Promise<void> {
    if (!this.audioContext) return;

    try {
      const promises = this.soundUrls.map(async (url) => {
        const response = await fetch(url);
        const arrayBuffer = await response.arrayBuffer();
        return this.audioContext!.decodeAudioData(arrayBuffer);
      });

      this.soundBuffers = await Promise.all(promises);
    } catch (e) {
      console.warn("音效加载失败:", e);
    }
  }

  // 播放随机音效
  private playRandomSound(): void {
    const uiStore = useUIStore();
    if (
      !uiStore.enableChatEffectSound ||
      !this.audioContext ||
      this.soundBuffers.length === 0
    ) {
      return;
    }

    const buffer =
      this.soundBuffers[Math.floor(Math.random() * this.soundBuffers.length)];
    const source = this.audioContext.createBufferSource();
    source.buffer = buffer;

    // 添加一些随机变化使音效更自然
    source.playbackRate.value = 1.0;

    const gainNode = this.audioContext.createGain();
    gainNode.gain.value = 0.8; // 降低音量

    source.connect(gainNode);
    gainNode.connect(this.audioContext.destination);

    source.start();
  }

  // 设置是否启用音效
  public setSoundEnabled(enabled: boolean): void {
    const uiStore = useUIStore();
    uiStore.enableChatEffectSound = enabled;
    if (enabled && !this.audioContext) {
      this.initAudio();
    }
  }

  public async start(text: string, numSpeed?: number): Promise<void> {
    const uiStore = useUIStore();
    // 停止上一个动画
    this.stop();
    this.isFinished = false;

    // 处理速度参数
    if (numSpeed !== undefined) {
      this.speed = Number.isInteger(numSpeed)
        ? numSpeed
        : Number.parseInt(numSpeed.toString(), 10) || 50;
    } else {
      console.warn("速度值未定义，将采用默认速度。");
      this.speed = 50;
    }

    this.abortController = new AbortController();
    let i = 0;
    // this.element.value = "";
    // this.element.textContent = "";
    this.textBuffers = "";

    // 如果启用音效但音频未初始化，则初始化
    if (uiStore.enableChatEffectSound && !this.audioContext) {
      await this.initAudio();
    }

    const typing = (): void => {
      if (this.abortController?.signal.aborted) {
        return;
      }

      if (i < text.length) {
        this.textBuffers += text.charAt(i);
        this.element.value = this.textBuffers;
        this.element.textContent = this.textBuffers;
        this.playRandomSound();
        i++;
        this.element.scrollTop = this.element.scrollHeight;

        // 计算随机间隔时间
        const baseDelay = this.speed * 0.8; // 基础速度为设定值的一半
        const randomVariation = this.speed * 0.4; // 随机变化范围为设定值的一半
        const delay = baseDelay + Math.random() * randomVariation;

        this.timer = setTimeout(typing, delay);
      } else {
        this.finish();
      }
    };

    // 立即开始第一次输入
    typing();
  }

  public finish(): void {
    this.stop();
    this.isFinished = true;
    (this.element.style as any).borderRight = "none";
    if (this.onFinishCallback) {
      this.onFinishCallback();
    }
  }

  public stop(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
    this.isFinished = false;
  }

  public clear(): void {
    this.element.value = "";
    this.element.textContent = "";
    this.textBuffers = "";
  }

  public checkFinished(): boolean {
    return this.isFinished;
  }

  public onFinish(callback: () => void): void {
    this.onFinishCallback = callback;
  }
}
