import { API_URLS } from "../consts.ts";
import { IEffectConfig } from "../types/Effect.ts";
import { ModelInfo } from "../types/ModelInfo.ts";
import { SettingNode1 } from "../types/settings.ts";
import { Locale } from "./I18N.ts";
import { send } from "./message.ts";
import ThrowHelper from "./ThrowHelper.ts";
import { UserInfo } from "./UserInfo.ts";

export interface Defaults {
    model: ModelInfo;
}
export interface Settings {
    ui_lang: Locale;
    text_speed: number;
    effects: { [effect_name: string]: IEffectConfig };
}

export function createDefaultsStatic(): Defaults {
    return <Defaults>{
        model: null!
    };
}

export function createSettingsStatic(): Settings {
    return <Settings>{
        ui_lang: "zh",
        text_speed: 300
    };
}

export async function initSettings(user_info: UserInfo): Promise<Settings> {
    return send(API_URLS.SETTINGS, {
        id: user_info.id,
        auth_token: user_info.auth_token
    }).then(response => {
        return response.data as Settings;
    });
}

export async function initDefaults(user_info: UserInfo): Promise<Defaults> {
    return send(API_URLS.DEFAULTS, {
        id: user_info.id,
        auth_token: user_info.auth_token
    }).then(response => {
        if (response.status != 200) {
            ThrowHelper(response.statusText);
        }
        return response.data as Defaults;
    });
}

export async function getSettingsStructure(): Promise<SettingNode1> {
    return send(API_URLS.SETTINGS).then(response => {
        if (response.status != 200) {
            ThrowHelper(response.statusText);
        }
        return response.data as SettingNode1;
    });
}
