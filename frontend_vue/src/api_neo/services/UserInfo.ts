import send from "./message.ts";
import { API_URLS } from "../consts.ts";
import { CharacterCard, CharacterCardCover } from "../types/CharacterCard.ts";

export interface UserData {
    readonly isInitialized: boolean;
    current_card: CharacterCard; // 当前使用中的角色卡
    available_cards: CharacterCardCover[]; // 所有当前可用角色卡的id
}

export interface UserInfo {
    readonly isInitialized: boolean;
    id: number;
    name: string;
    auth_token: string;
}

export function createUserDataStatic(): UserData {
    return <UserData>{
        get isInitialized() {
            return this.current_card != null && this.available_cards != null;
        },
        current_card: null!,
        available_cards: null!
    };
}

export function createUserInfoStatic(): UserInfo {
    return <UserInfo>{
        get isInitialized() {
            return this.id != null && this.name != null && this.auth_token != null;
        },
        id: null!,
        name: null!,
        auth_token: null!
    };
}

export function loadUserInfo(user_info: UserInfo, user_id: number, user_name: string, user_auth_token: string) {
    user_info.id = user_id;
    user_info.name = user_name;
    user_info.auth_token = user_auth_token;
}

export async function getUserInfo(user_info: UserInfo, username: string, encrypted_password: string) {
    return send(API_URLS.USER_INFO, {
        username: username,
        password: encrypted_password
    }).then(response => {
        if (response.status != 200) {
            throw new Error(response.statusText);
        }
        loadUserInfo(user_info, response.data.id, response.data.name, response.data.auth_token);
    });
}

export function passwordEncrypt(password: string) {
    console.error("password encrypt is not implemented"); // TODO
    return password;
}

export function loadUserData(user_data: UserData, current_card: CharacterCard, available_cards: CharacterCardCover[]) {
    user_data.current_card = current_card;
    user_data.available_cards = available_cards;
}

export async function getUserData(user_info: UserInfo, user_data: UserData) {
    return send(API_URLS.USER_DATA, {
        id: user_info.id,
        auth_token: user_info.auth_token
    }).then(response => {
        user_data.current_card = response.data.current_card as CharacterCard;
        user_data.available_cards = response.data.available_cards as CharacterCardCover[];
    });
}
