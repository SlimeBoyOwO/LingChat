// 统一DOM元素引用
export const DOM = {
  // Galgame 普通对话模式 Container
  galNormalContainer: document.getElementById("gal-normal") as HTMLInputElement,

  // 输入区域
  input: document.getElementById("inputMessage") as HTMLInputElement,
  sendBtn: document.getElementById("sendButton") as HTMLButtonElement,

  // 状态显示
  status: document.getElementById("status") as HTMLElement,
  audioStatus: document.getElementById("audioStatus") as HTMLElement,

  // 头像区域
  avatar: {
    img: document.querySelector(".main-box img") as HTMLImageElement,
    container: document.querySelector(".avatar-container") as HTMLElement,
    title: document.getElementById("character") as HTMLElement,
    subtitle: document.getElementById("character-sub") as HTMLElement,
    emotion: document.getElementById("character-emotion") as HTMLElement,
    bubble: document.querySelector(".bubble") as HTMLElement,
  },

  // 音频
  audioPlayer: document.getElementById("audioPlayer") as HTMLAudioElement,
  bubbleAudio: document.getElementById("audioPlayerBubble") as HTMLAudioElement,
  effectAudio: document.getElementById("audioPlayerEffect") as HTMLAudioElement,

  BackAudioPlayer: document.getElementById(
    "BackAudioPlayer"
  ) as HTMLAudioElement,
  VolumeAudioPlayer: document.getElementById(
    "VolumeAudioPlayer"
  ) as HTMLInputElement,
  VolumeBubbleAudio: document.getElementById(
    "VolumeBubbleAudio"
  ) as HTMLInputElement,
  VolumeBackAudioPlayer: document.getElementById(
    "VolumeBackAudioPlayer"
  ) as HTMLInputElement,
  TestAudioPlayer: document.getElementById(
    "testAudioPlayer"
  ) as HTMLAudioElement,
  TestBubbleAudio: document.getElementById(
    "testBubbleAudio"
  ) as HTMLAudioElement,
  PlayPauseMusic: document.getElementById(
    "play-pause-music"
  ) as HTMLButtonElement,
  StopMusic: document.getElementById("stop-music") as HTMLButtonElement,
  MusicName: document.getElementById("music-name") as HTMLElement,
  MusicList: document.getElementById("Music-list") as HTMLSelectElement,
  AddMusic: document.getElementById("add-music") as HTMLButtonElement,
  MusicUpload: document.getElementById("music-upload") as HTMLInputElement,

  // 特效背景
  canvas: document.getElementById("canvas") as HTMLCanvasElement,
  fgEffect: document.getElementById("frontpage-effect") as HTMLElement,

  // 菜单部分
  menuToggle: document.getElementById("menu-toggle") as HTMLButtonElement,
  menuContent: document.getElementById("menu-content") as HTMLElement,
  closeMenu: document.getElementById("close-menu") as HTMLButtonElement,

  menuCharacter: document.getElementById("menu-character") as HTMLButtonElement,
  characterPage: document.getElementById("character-page") as HTMLElement,

  menuText: document.getElementById("menu-text") as HTMLButtonElement,
  textPage: document.getElementById("text-page") as HTMLElement,

  menuImage: document.getElementById("menu-image") as HTMLButtonElement,
  imagePage: document.getElementById("background-page") as HTMLElement,

  menuSound: document.getElementById("menu-sound") as HTMLButtonElement,
  soundPage: document.getElementById("sound-page") as HTMLElement,

  menuSave: document.getElementById("menu-save") as HTMLButtonElement,
  savePage: document.getElementById("save-page") as HTMLElement,

  // 文本部分
  text: {
    speedInput: document.getElementById("speed-option") as HTMLInputElement,
    testMessage: document.getElementById("testmessage") as HTMLInputElement,
    soundEffectToggle: document.getElementById(
      "sound-effect-toggle"
    ) as HTMLInputElement,
  },

  // 菜单历史部分
  history: {
    toggle: document.getElementById("menu-history") as HTMLButtonElement,
    content: document.getElementById("history-page") as HTMLElement,
    list: document.getElementById("history-list") as HTMLElement,
    clearBtn: document.getElementById("clear-history") as HTMLButtonElement,
  },

  // 角色部分
  character: {
    refreshCharactersBtn: document.getElementById(
      "refresh-characters-btn"
    ) as HTMLButtonElement,
    openWebBtn: document.getElementById("open-web-btn") as HTMLButtonElement,
  },

  // 图像部分
  image: {
    // 预览部分
    qinling: document.getElementById("qinling") as HTMLElement,
    qinlingtest: document.getElementById("qinlingtest") as HTMLButtonElement,
    filterKuosan: document.getElementById("filter-kuosan") as HTMLSelectElement,

    // 添加自定义背景功能
    uploadBgBtn: document.getElementById("upload-bg-btn") as HTMLButtonElement,
    uploadBgInput: document.getElementById(
      "upload-bg-input"
    ) as HTMLInputElement,
    uploadStatus: document.getElementById("upload-status") as HTMLElement,
    bgOption: document.querySelectorAll(
      ".bg-option"
    ) as NodeListOf<HTMLElement>,
    bgOptions: document.querySelector(".bg-options") as HTMLElement,

    kuosanPreview: document.getElementById("kuosan-preview") as HTMLElement,
    kousanPreviewImg: document.getElementById(
      "kuosan-preview-img"
    ) as HTMLImageElement,
    kuosanTest: document.getElementById("kuosan-test") as HTMLButtonElement,
  },

  // 存档部分
  save: {
    uploadBtn: document.getElementById("upload-btn") as HTMLButtonElement,
    fileInput: document.getElementById("log-upload") as HTMLInputElement,
  },
};

/**
 * 安全元素访问
 * @param key DOM元素的键名
 * @returns DOM元素或null
 */
export const getSafeDOM = <K extends keyof typeof DOM>(
  key: K
): (typeof DOM)[K] | null => {
  const el = DOM[key];
  if (!el) {
    console.warn(`DOM元素未找到: ${String(key)}`);
  }
  return el || null;
};

/**
 * 批量获取DOM元素
 * @param selectors 选择器对象
 * @returns DOM元素对象
 */
export const getDOMElements = <T extends Record<string, string>>(
  selectors: T
): { [K in keyof T]: Element | null } => {
  const elements = {} as { [K in keyof T]: Element | null };

  for (const [key, selector] of Object.entries(selectors)) {
    elements[key as keyof T] = document.querySelector(selector);
  }

  return elements;
};
