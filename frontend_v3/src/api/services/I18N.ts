import { Translation } from "../../i18n/TranslationBase";
import zh from "../../i18n/zh";
import ThrowHelper from "./ThrowHelper";

export enum Locale {
    en = "en",
    zh = "zh"
}
export interface I18N {
    __locale: Locale;
    __fallback: Locale;
    __locales: { [key in Locale]?: Translation };
    readonly current: Translation;
    readonly fallback: Translation;
    set: (locale: Locale, fallback?: Locale) => this;
    get: (key: string) => string;
}

export function createI18NStatic(): I18N {
    return {
        __locale: Locale.zh,
        __fallback: Locale.zh,
        __locales: {
            [Locale.zh]: zh
        },
        get current() {
            return this.__locales[this.__locale] ?? {};
        },
        get fallback() {
            return this.__locales[this.__fallback] ?? {};
        },
        set(locale, fallback = Locale.zh) {
            this.__locale = locale;
            this.__fallback = fallback;
            return this;
        },
        get(key) {
            if (key in this.current) {
                return this.current[key];
            }
            console.warn(`No translation for ${key} was found in ${this.__locale}. Trying fallback.`);
            if (key in this.fallback) {
                return this.fallback[key];
            }
            ThrowHelper(`No translation for ${key} was found in both ${this.__locale} and ${this.__fallback}`);
        }
    };
}
