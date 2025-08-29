import { CONFIG } from "../consts";
import { settings } from "../store.ts";
import { IEffect, IEffectEmpty } from "../types/Effect.ts";


export interface UIStatus {
    __nav_stack: string[];
    readonly currentPage: string;
    switchPage: <T = this>(page: string) => T;
    back: <T = this>() => T;
}

export interface SingleAudio {
    src: string;
    volume?: number;
    loop?: boolean;
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
    readonly text: { [key: string]: string };
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
    return <UIStatus>{
        __nav_stack: [beginPage],
        isLoading: true,
        isFastLoad: false,
        __load_progress: 0,
        get currentPage() {
            if (this.__nav_stack.length === 0) {
                throw new Error("nav_stack is empty.");
            }
            return this.__nav_stack[this.__nav_stack.length - 1];
        },
        switchPage(page: string) {
            if (this.__nav_stack.length > 0 && this.currentPage === page) {
                return this;
            }
            const index = this.__nav_stack.indexOf(page);
            if (index !== -1) {
                this.__nav_stack.splice(index + 1);
            }
            return this;
        },
        back() {
            this.__nav_stack.pop();
            return this;
        }
    };
}

export function extendToRootUIStatus(uiStatus: UIStatus): RootUIStatus {
    return <RootUIStatus>{
        ...uiStatus,
        background_image: CONFIG.DEFAULT_BACKGROUND,
        background_effect: IEffectEmpty,
        effect: IEffectEmpty,
        get loadProgress() {
            return this.__load_progress;
        },
        get text() {
            return I18N[settings.value.ui_lang]??I18N["zh"];
        }
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
    };
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
            if (factor < 0) throw new Error("Animation duration factor cannot be negative.");
            this.__factor = factor;
            return this;
        },
        setDuration(duration: number) {
            if (duration < 0) throw new Error("Animation duration cannot be negative.");
            this.__duration = duration;
            return this;
        },
        setMinDuration(duration: number) {
            if (duration < 0) throw new Error("Animation duration cannot be negative.");
            if (this.__max_duration && duration > this.__max_duration)
                throw new Error("Minimum animation duration cannot be bigger than maximum animation duration.");
            this.__min_duration = duration;
            return this;
        },
        setMaxDuration(duration: number) {
            if (duration < 0) throw new Error("Animation duration cannot be negative.");

            if (duration < this.__min_duration)
                throw new Error("Maximum animation duration cannot be smaller than minimum animation duration.");
            this.__max_duration = duration;
            return this;
        }
    };
}
