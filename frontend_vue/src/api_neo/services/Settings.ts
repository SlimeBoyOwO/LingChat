import send from "./message.ts";
import { API_URLS } from "../consts.ts";
import { ModelInfo } from "../types/ModelInfo.ts";
import { UserInfo } from "./UserInfo.ts";
export interface Defaults {
    model: ModelInfo;
}
export interface Settings {}

export function createDefaultsStatic(): Defaults {
    return <Defaults>{
        model: null!
    };
}

export function createSettingsStatic(): Settings {
    return <Settings>{};
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
            throw new Error(response.statusText);
        }
        return response.data as Defaults;
    });
}
