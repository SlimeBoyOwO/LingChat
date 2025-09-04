import { CONFIG } from "../consts";
import { IEffect, IEffectEmpty } from "../types/Effect.ts";
import ThrowHelper from "./ThrowHelper.ts";

export interface UIStatus {
    __nav_stack: string[];
    __with?: unknown; //存储附加参数
    readonly currentPage: string;
    readonly canBack: boolean;
    switchPage: <T = this>(page: string) => T;
    with: <T extends object, This = this>(_with: T) => This; //在切换页面时传递附加参数
    read: <T extends object>() => T | undefined; //读取附加参数
    back: <T = this>() => T; //返回上一页
}

export interface SingleAudio {
    src: string;
    volume: number;
    loop: boolean;
    onEnded: () => void;
}

interface GlobalAudios {
    effect?: SingleAudio;
    background?: SingleAudio;
    avatar?: SingleAudio;
    voice?: SingleAudio;
}

export interface RootUIStatus extends UIStatus {
    background_image: string;
    background_effect: IEffect;
    effect: IEffect;
    audio: GlobalAudios;
    isLoading: boolean;
    isFastLoad: boolean;
    __load_progress: number;
    readonly loadProgress: number;
    beginLoading: <T = this>(fast_load?: boolean) => T;
    endLoading: <T = this>() => T;
    setLoadProgress: <T = this>(progress: number, relative?: boolean) => T;
}

export interface AnimationData {
    __min_duration: number;
    __max_duration?: number;
    __duration: number;
    __factor: number;
    readonly __raw_duration: number;
    readonly duration: number;
    readonly factor: number;
    setFactor: (factor: number) => this;
    setMinDuration: (duration: number) => this;
    setMaxDuration: (duration: number) => this;
    setDuration: (duration: number) => this;
}

export function createUIStatusStatic(beginPage: string): UIStatus {
    const _ui_status = <UIStatus>{
        __nav_stack: [beginPage],
        __with: undefined,
        isLoading: true,
        isFastLoad: false,
        __load_progress: 0,
        get currentPage() {
            return this.__nav_stack[this.__nav_stack.length - 1];
        },
        get canBack() {
            return this.__nav_stack.length > 1;
        },
        switchPage(page: string) {
            this.__with = undefined;
            if (this.__nav_stack.length > 0 && this.currentPage === page) {
                return this;
            }
            const index = this.__nav_stack.indexOf(page);
            if (index !== -1) {
                this.__nav_stack.splice(index);
            }
            this.__nav_stack.push(page);
            return this;
        },
        with<T extends object>(_with: T) {
            this.__with = _with;
            return this;
        },
        read<T extends object>() {
            const temp = this.__with;
            return temp as T;
        },
        back() {
            if (this.canBack) {
                this.__nav_stack.pop();
            }
            return this;
        }
    };
    return _ui_status;
}

export function extendToRootUIStatus(uiStatus: UIStatus): RootUIStatus {
    Object.assign(uiStatus, <RootUIStatus>{
        background_image: CONFIG.DEFAULT_BACKGROUND,
        background_effect: IEffectEmpty,
        effect: IEffectEmpty,
        get loadProgress() {
            return this.__load_progress;
        },
        beginLoading(fast_load: boolean = false) {
            this.__load_progress = 0;
            this.isLoading = true;
            this.isFastLoad = fast_load;
            return this;
        },
        endLoading(ensure: boolean = true) {
            if (ensure) this.__load_progress = 100;
            this.isLoading = false;
            this.isFastLoad = false;
            return this;
        },
        setLoadProgress(progress: number, relative: boolean = false) {
            const new_progress = relative ? progress : this.__load_progress + progress;
            this.__load_progress = new_progress < 0 ? 0 : new_progress > 100 ? 100 : new_progress;
            return this;
        },
        audio: <GlobalAudios>{}
    });
    return uiStatus as RootUIStatus;
}

export function createNewAnimationData(
    duration: number,
    max_duration?: number,
    min_duration: number = 0,
    factor: number = 1 // 速度因子只会影响 duration，不会影响最值。
): AnimationData {
    return {
        __min_duration: min_duration,
        __max_duration: max_duration,
        __duration: duration,
        __factor: factor,
        get factor() {
            return this.__factor;
        },
        get __raw_duration() {
            return this.__duration * this.__factor;
        },
        get duration() {
            if (this.__max_duration && this.__raw_duration > this.__max_duration) return this.__max_duration;

            if (this.__min_duration && this.__raw_duration < this.__min_duration) return this.__min_duration;
            return this.__raw_duration;
        },
        setFactor(factor: number) {
            if (factor < 0) ThrowHelper("Animation duration factor cannot be negative.");
            this.__factor = factor;
            return this;
        },
        setDuration(duration: number) {
            if (duration < 0) ThrowHelper("Animation duration cannot be negative.");
            this.__duration = duration;
            return this;
        },
        setMinDuration(duration: number) {
            if (duration < 0) ThrowHelper("Animation duration cannot be negative.");
            if (this.__max_duration && duration > this.__max_duration)
                ThrowHelper("Minimum animation duration cannot be bigger than maximum animation duration.");
            this.__min_duration = duration;
            return this;
        },
        setMaxDuration(duration: number) {
            if (duration < 0) ThrowHelper("Animation duration cannot be negative.");

            if (duration < this.__min_duration)
                ThrowHelper("Maximum animation duration cannot be smaller than minimum animation duration.");
            this.__max_duration = duration;
            return this;
        }
    };
}
