import axios from "axios";
import { API_URLS } from "../consts.ts";
import { CharacterCard,CharacterCardCover } from "../types/CharacterCard.ts";
import { ModelInfo } from "../types/ModelInfo.ts";
import { UserInfo } from "./UserInfo.ts";
export interface Defaults {
    model: ModelInfo;
}
export interface Settings {
    current_card: CharacterCard; //当前正在使用的角色卡
    characterCards: CharacterCardCover[]; //所有用过的角色卡的id
}

export async function initSettings(user_info: UserInfo): Promise<Settings> {
    return axios
        .post(API_URLS.SETTINGS, {
            id: user_info.id,
            auth_token: user_info.auth_token
        })
        .then(response => {
            return response.data as Settings;
        })
        .catch((error: Error) => {
            console.error("Error fetching settings:", error);
            throw error;
        });
}

export async function initDefaults(user_info:UserInfo): Promise<Defaults> {
    return axios
        .post(API_URLS.DEFAULTS, {
            id: user_info.id,
            auth_token:user_info.auth_token
        })
        .then(response => {
            if (response.status != 200) {
                throw new Error(response.statusText);
            }
            return response.data as Defaults;
        })
        .catch((error: Error) => {
            console.error("Error fetching defaults:", error);
            throw error;
        });
}
