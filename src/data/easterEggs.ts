// ============================================================
//  彩蛋文字数据模块
//  集中管理 LoadingTransition 中展示的加载台词 & 随机小贴士
//  方便日后编辑和维护
// ============================================================

// ---- 加载状态台词（在 50% / 70% / 90% 时随机切换） ----
export const statusTexts: string[] = [
  '正在校准量子猫爪频率...',
  'TCP 握爪中，请不要松开~',
  '心灵链接建立中，请耐心等待...',
  '正在唤醒终端小可爱...',
  '加载猫薄荷驱动中...',
  '同步星际中继站信号...',
  '正在烘焙虚拟小饼干...',
  '调整全息投影参数中...',
  '喵喵喵，马上就好啦~',
  '正在连接喵星服务器...',
  '激光笔定位校准完毕！',
  '检测到大量可爱粒子，收集中...',
]

/** 从 statusTexts 中随机选取一条不同于 currentText 的台词 */
export function pickRandomStatusText(currentText: string): string {
  const unused = statusTexts.filter((s) => s !== currentText)
  if (unused.length === 0) return currentText
  return unused[Math.floor(Math.random() * unused.length)]
}

// ---- 随机小贴士（按权重分类） ----
export interface TipCategory {
  name: string
  weight: number
  messages: string[]
}

export const tipCategories: TipCategory[] = [
  {
    name: '游戏提示',
    weight: 0.4,
    messages: [
      '欢迎使用 LingChat 终端机 (*^▽^*)',
      'LingChat 正在努力加载中... 稍等一下哦',
      '看进度条会变傻，不许看！',
      '你有打开过久坐提醒功能吗？工作党必须哦~',
      '桌宠模式下的代办功能很方便哦，试试看吧~',
      '欢迎向创意工坊投稿自己制作的角色！',
      '你可以在设置中更改角色和背景哦~',
      '你可以为背景添加相关提示词让它可以被感知哦~',
      '你知道羁绊剧情是可以自己做的吗？试试看吧！',
      '通用板块有很多实用的功能，去试试捏',
      '和 Galgame 一样，不要忘记存档！',
      '别忘了打开永久记忆功能呢',
      '为了你的沉浸式体验，可以加自己喜欢的音乐和环境音哦',
      '在背景界面有很多可以设置的部分，个性化你的游戏去吧！',
      '欢迎前往创意工坊投稿你的角色和剧情！',
      'LingChat 的角色表情是通过深度学习推理出情绪的哦，厉害吧！',
      '永久记忆会自动压缩记忆，放心啦~',
      'LingChat 的运行逻辑比你想象中复杂很多呢，快去探索吧！',
      '发现那个照相机了吗？你可以截图给她看东西的哦！',
      '可以用麦克风直接和她语音对话呢',
      '用你的小爪爪可以摸摸她~',
      '摁下 F11 可以打开全屏模式哦',
      '使用 Ctrl + 鼠标滚轮 可以缩放界面大小哦',
    ],
  },
  {
    name: '求情广告',
    weight: 0.1,
    messages: [
      '给LingChat点点star喵，给LingChat点点star谢谢喵',
      '灌注钦灵喵，灌注钦灵谢谢喵~',
      '钦灵这么可爱，真的不给她点个star吗？',
      '开发者都是学生哦，软件制作不易~',
      '你有注意到致谢吗？这是社区的力量哦！',
      '开发者缺人啦！欢迎想要参与开发的人加入哦！',
      'LingChat 的开源社区的每一个贡献者都很辛苦哦...',
      'LingChat 的 Star 数量超过 1000 了！好耶！',
    ],
  },
  {
    name: '开发者彩蛋',
    weight: 0.2,
    messages: [
      '你知道吗？钦灵本人比AI钦灵更可爱（也更淫荡）',
      '其实风雪并不会写代码，她只是趴在键盘上睡着了，然后恰好对LingChat提交了commit',
      '影空正在被钦灵催情变得越来越淫荡...',
      '正在加载PL的代码... 等等，python 已经被换成 rust 了...',
      '你们看到云小姐了吗？嗯嗯，她没有失踪也没有怎么样的，我们只是想让你知道，她很可爱',
      '喵？喵~ 喵！',
      'uwa是萝莉音，望周知',
      '七毛钱的苹果？好吃，耶！',
      '大饼鸡蛋一听就很好吃的样子，AstroBot 也有他哦！',
      '喵本喵正在努力的学习画画以此变为项目的黑奴...',
      '开发者群最淫荡的七辰正在用户群穿胖次外漏乱跑中',
      '钦灵正在努力的改写有梦当然留下的组件的代码 >A<',
      '波奶很可爱~啵啵~❤',
      '饲养员莱尔正在努力供养开发者群...',
      '诺亚狐和七辰喜欢管理着文档，也喜欢开银趴',
      '魔法少女总督挥动了神奇的魔法棒，让你看到了奇妙的鼠标粒子特效~',
      'Heiyaha正在视奸你的CPU',
      '哦~卷大人，你是一只可爱的猫娘喵~',
      '徒花花可爱爱，红瞳长发令人爱~',
      '晚安喜欢被钦灵用鞭子抽着屁股干活，嘿嘿嘿',
      '远足正在从钦灵疯狂的偷猫图，可恶！',
      '45454，用 10M token 感动了钦灵做安卓版本，快谢谢他',
      '元初是个小男娘，喜欢艾草',
      'RatMan 挖了个地洞偷偷跑了...',
      '琉璃子非常喜欢白丝，我们希望你能知道，之后可以送他一份',
      'Flame 小姐的代码和她一样美丽',
      'Yukito柏海和梦轩一起创造了你看到的美丽的UI！',
      'Matsuko 喜欢被榨奶... 狐狐澪爱看❤',
    ],
  },
]

/** 按权重随机选分类，再从分类中随机选一条消息 */
export function pickRandomTip(): string {
  const totalWeight = tipCategories.reduce((sum, c) => sum + c.weight, 0)
  let r = Math.random() * totalWeight
  for (const cat of tipCategories) {
    r -= cat.weight
    if (r <= 0) {
      return cat.messages[Math.floor(Math.random() * cat.messages.length)]
    }
  }
  // fallback
  return tipCategories[0].messages[0]
}
