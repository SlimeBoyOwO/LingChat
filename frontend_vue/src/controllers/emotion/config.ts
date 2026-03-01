import { API_CONFIG } from '../core/config.js'

interface EmotionConfig {
  animation: string
  bubbleImage: string
  bubbleClass: string
  audio: string
  avatar: string
}

interface EmotionConfigMap {
  [key: string]: EmotionConfig
}

interface EmotionMap {
  [key: string]: string
}

export const EMOTION_CONFIG_EMO: EmotionMap = {
  厌恶: '厌恶',
  高兴: '高兴',
  担心: '担心',
  生气: '生气',
  紧张: '紧张',
  害怕: '害怕',
  害羞: '害羞',
  慌张: '慌张',
  认真: '认真',
  无奈: '无奈',
  兴奋: '兴奋',
  疑惑: '疑惑',
  哭泣: '伤心',
  心动: '心动',
  调皮: '调皮',
  难为情: '羞耻',
  自信: '自信',
  惊讶: '惊讶',
  正常: '正常',
  平静: '平静',
  AI思考: 'none',
}

export const EMOTION_CONFIG: EmotionConfigMap = {
  厌恶: {
    animation: 'none',
    bubbleImage: '../pictures/animation/生气.webp',
    bubbleClass: 'angry',
    audio: '../audio_effects/厌恶.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/厌恶.png`,
  },
  高兴: {
    animation: 'happy-bounce',
    bubbleImage: '../pictures/animation/高兴.webp',
    bubbleClass: 'happy',
    audio: '../audio_effects/喜悦.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/高兴.png`,
  },
  担心: {
    animation: 'none',
    bubbleImage: '../pictures/animation/流泪.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/伤心.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/担心.png`,
  },
  生气: {
    animation: 'angry-jump',
    bubbleImage: '../pictures/animation/生气2.webp',
    bubbleClass: 'angry',
    audio: '../audio_effects/生气.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/生气.png`,
  },
  紧张: {
    animation: 'none',
    bubbleImage: '../pictures/animation/紧张.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/尴尬.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/紧张.png`,
  },
  害怕: {
    animation: 'none',
    bubbleImage: '../pictures/animation/惊讶.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/震惊.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/害怕.png`,
  },
  害羞: {
    animation: 'none',
    bubbleImage: '../pictures/animation/害羞.webp',
    bubbleClass: 'shy',
    audio: '../audio_effects/害羞.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/害羞.png`,
  },
  慌张: {
    animation: 'none',
    bubbleImage: '../pictures/animation/慌乱.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/震惊.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/慌张.png`,
  },
  认真: {
    animation: 'serious-think',
    bubbleImage: 'none',
    bubbleClass: 'none',
    audio: 'none',
    avatar: `${API_CONFIG.AVATAR.BASE}/认真.png`,
  },
  无奈: {
    animation: 'none',
    bubbleImage: '../pictures/animation/叹气.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/叹气.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/无奈.png`,
  },
  兴奋: {
    animation: 'happy-bounce',
    bubbleImage: '../pictures/animation/聊天.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/聊天.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/兴奋.png`,
  },
  疑惑: {
    animation: 'none',
    bubbleImage: '../pictures/animation/疑问.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/疑问.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/疑惑.png`,
  },
  哭泣: {
    animation: 'none',
    bubbleImage: '../pictures/animation/流泪.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/伤心.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/伤心.png`,
  },
  心动: {
    animation: 'heart-beat',
    bubbleImage: '../pictures/animation/心动.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/喜爱.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/心动.png`,
  },
  调皮: {
    animation: 'naughty-bounce',
    bubbleImage: '../pictures/animation/高兴.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/愉快.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/调皮.png`,
  },
  难为情: {
    animation: 'embarrassed-emo',
    bubbleImage: '../pictures/animation/难为情.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/察觉.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/羞耻.png`,
  },
  自信: {
    animation: 'none',
    bubbleImage: '../pictures/animation/高兴.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/愉快.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/自信.png`,
  },
  惊讶: {
    animation: 'none',
    bubbleImage: '../pictures/animation/惊讶.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/察觉.wav',
    avatar: `${API_CONFIG.AVATAR.BASE}/惊讶.png`,
  },
  平静: {
    animation: 'none',
    bubbleImage: 'none',
    bubbleClass: 'none',
    audio: 'none',
    avatar: `${API_CONFIG.AVATAR.BASE}/平静.png`,
  },
  正常: {
    animation: 'none',
    bubbleImage: 'none',
    bubbleClass: 'none',
    audio: 'none',
    avatar: `${API_CONFIG.AVATAR.BASE}/正常.png`,
  },
  AI思考: {
    animation: 'none',
    bubbleImage: '../pictures/animation/AI思考.webp',
    bubbleClass: 'none',
    audio: '../audio_effects/无语.wav',
    avatar: 'none',
  },
}

// 预加载资源
export const preloadAssets = (): void => {
  Object.values(EMOTION_CONFIG).forEach((config) => {
    if (config.avatar !== 'none') {
      new Image().src = config.avatar
    }
  })
}
