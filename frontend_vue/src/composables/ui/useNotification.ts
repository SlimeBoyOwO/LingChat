import { ref } from 'vue';

// 通知类型定义
export type NotificationType = 'error' | 'success' | 'info' | 'warning';

// 通知状态接口
export interface NotificationState {
    isVisible: boolean;
    type: NotificationType;
    title: string;
    message: string;
    avatarUrl: string;
    duration: number;
}

// 全局通知状态（单例）
const notificationState = ref<NotificationState>({
    isVisible: false,
    type: 'info',
    title: '',
    message: '',
    avatarUrl: '',
    duration: 3000,
});

// 提示映射（从 tips.txt 加载）
const tipsMap: Record<string, { title: string; message: string }> = {};
let tipsLoaded = false;
let tipsAvailable = false;  // 标记当前角色是否有 tips.txt
let currentCharacterFolder = '';  // 当前加载的角色文件夹名

// localStorage key
const STORAGE_KEY_CHARACTER_FOLDER = 'lingchat_character_folder';
const DEFAULT_CHARACTER_FOLDER = '诺一钦灵';

// 加载角色专属提示
async function loadTips(characterFolderName: string): Promise<boolean> {
    // 清空之前的提示
    Object.keys(tipsMap).forEach(key => delete tipsMap[key]);
    tipsLoaded = false;
    tipsAvailable = false;
    currentCharacterFolder = characterFolderName;

    // 保存到 localStorage（后端返回新角色时会更新）
    localStorage.setItem(STORAGE_KEY_CHARACTER_FOLDER, characterFolderName);

    try {
        const response = await fetch(`/characters/${characterFolderName}/tips.txt`);

        // 检查文件是否存在
        if (!response.ok) {
            console.log(`⚠️ 角色 ${characterFolderName} 没有 tips.txt，将不显示弹窗`);
            tipsLoaded = true;
            tipsAvailable = false;
            return false;
        }

        const text = await response.text();

        // 解析 txt 格式：代码 = 标题 | 内容
        text.split('\n').forEach(line => {
            line = line.trim();
            if (!line || line.startsWith('#')) return;

            const [code, content] = line.split('=').map(s => s.trim());
            if (code && content) {
                const [title, message] = content.split('|').map(s => s.trim());
                if (title && message) {
                    tipsMap[code] = { title, message };
                }
            }
        });

        tipsLoaded = true;
        tipsAvailable = true;
        console.log(`✅ 已加载角色 ${characterFolderName} 的提示:`, tipsMap);
        return true;
    } catch (error) {
        console.log(`⚠️ 加载角色 ${characterFolderName} 的提示失败，将不显示弹窗:`, error);
        tipsLoaded = true;
        tipsAvailable = false;
        return false;
    }
}

// 启动时从 localStorage 读取上次的角色，没有则使用默认
const savedCharacterFolder = localStorage.getItem(STORAGE_KEY_CHARACTER_FOLDER) || DEFAULT_CHARACTER_FOLDER;
loadTips(savedCharacterFolder);

// 根据错误代码获取提示
function getErrorTip(errorCode: string) {
    return tipsMap[errorCode] || tipsMap['default_error'] || {
        title: '错误',
        message: '发生了未知错误'
    };
}

// 获取角色切换提示
function getSwitchTip(type: 'success' | 'fail') {
    const key = type === 'success' ? 'switch_success' : 'switch_fail';
    return tipsMap[key] || {
        title: type === 'success' ? '切换成功' : '切换失败',
        message: type === 'success' ? '角色已切换' : '切换时出了问题'
    };
}

// 默认头像
const DEFAULT_AVATAR = '/characters/诺一钦灵/头像.png';

// 防抖相关变量（放在函数外部保持状态）
const notificationDebounceMap = new Map<string, number>();  // key: title+message, value: lastShowTime
const DEBOUNCE_MS_NETWORK = 10000;  // 网络/500错误：10秒
const DEBOUNCE_MS_DEFAULT = 3000;   // 其他错误：3秒

export function useNotification() {
    let hideTimer: number | null = null;

    /**
     * 显示通知（通用方法）
     */
    const show = (options: {
        type?: NotificationType;
        title?: string;
        message?: string;
        avatarUrl?: string;
        duration?: number;
    }) => {
        // 如果当前角色没有配置 tips.txt，完全不显示弹窗
        if (!tipsAvailable) {
            console.log('跳过弹窗：当前角色没有配置 tips.txt');
            return;
        }

        const { type = 'info', title = '', message = '', avatarUrl, duration = 3000 } = options;

        const now = Date.now();
        const notificationKey = `${title}:${message}`;  // 用标题+内容作为唯一标识

        // 判断是否为"未注明的错误"，使用更长的防抖时间
        const isDefaultError = title === '未注明的错误';
        const debounceMs = isDefaultError ? DEBOUNCE_MS_NETWORK : DEBOUNCE_MS_DEFAULT;

        // 防抖：相同内容的通知在防抖时间内不重复显示
        const lastTime = notificationDebounceMap.get(notificationKey) || 0;
        if (now - lastTime < debounceMs) {
            console.log(`跳过重复通知：${title}（${debounceMs / 1000}秒内已显示过）`);
            return;
        }

        notificationDebounceMap.set(notificationKey, now);

        // 清除之前的定时器
        if (hideTimer) {
            clearTimeout(hideTimer);
        }

        notificationState.value = {
            isVisible: true,
            type,
            title,
            message,
            avatarUrl: avatarUrl || DEFAULT_AVATAR,
            duration,
        };

        // 自动隐藏
        if (duration > 0) {
            hideTimer = window.setTimeout(() => {
                hide();
            }, duration);
        }
    };

    /**
     * 隐藏通知
     */
    const hide = () => {
        notificationState.value.isVisible = false;
        if (hideTimer) {
            clearTimeout(hideTimer);
            hideTimer = null;
        }
    };

    /**
     * 显示错误通知（支持错误代码自动翻译）
     */
    const showError = (options: {
        errorCode?: string;
        statusCode?: number;
        title?: string;
        message?: string;
        avatarUrl?: string;
        duration?: number;
    }) => {
        const { errorCode, statusCode, title, message, avatarUrl, duration = 3000 } = options;

        let finalTitle = title || '错误';
        let finalMessage = message || '发生了未知错误';

        // 优先使用错误代码查询
        if (errorCode) {
            const tip = getErrorTip(errorCode);
            finalTitle = title || tip.title;
            finalMessage = message || tip.message;
        }
        // 其次使用 HTTP 状态码
        else if (statusCode) {
            const code = statusCode.toString();
            const httpCode = code + '_http';
            const tip = getErrorTip(httpCode) || getErrorTip(code);
            if (tip) {
                finalTitle = title || tip.title;
                finalMessage = message || tip.message;
            }
        }

        show({
            type: 'error',
            title: finalTitle,
            message: finalMessage,
            avatarUrl,
            duration,
        });
    };

    /**
     * 显示成功通知
     */
    const showSuccess = (options: {
        title?: string;
        message?: string;
        avatarUrl?: string;
        duration?: number;
    }) => {
        show({ ...options, type: 'success' });
    };

    /**
     * 显示信息通知
     */
    const showInfo = (options: {
        title?: string;
        message?: string;
        avatarUrl?: string;
        duration?: number;
    }) => {
        show({ ...options, type: 'info' });
    };

    /**
     * 显示警告通知
     */
    const showWarning = (options: {
        title?: string;
        message?: string;
        avatarUrl?: string;
        duration?: number;
    }) => {
        show({ ...options, type: 'warning' });
    };

    return {
        notificationState,
        show,
        hide,
        showError,
        showSuccess,
        showInfo,
        showWarning,
        loadTips,
        getSwitchTip,
        get tipsAvailable() { return tipsAvailable; },
    };
}
