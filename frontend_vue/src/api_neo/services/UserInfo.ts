import axios from "axios";
import { CharacterCard } from "../types/CharacterCard.ts";
import { API_URLS } from "../consts.ts";
export interface UserInfo {
    id: number;
    name: string;
    auth_token: string;
}
export function initUserInfo() {
    return <UserInfo>{
        id: undefined!,
        name: undefined!,
        auth_token: undefined!
    };
}

export function loadUserInfo(user_info: UserInfo, user_id: number, user_name: string, user_auth_token: string) {
    user_info.id = user_id;
    user_info.name = user_name;
    user_info.auth_token = user_auth_token;
}

export async function getUserInfo(user_info: UserInfo, username: string, encrypted_password: string) {
    return axios
        .post(API_URLS.USER_INFO, {
            username: username,
            password: encrypted_password
        })
        .then(response => {
            if (response.status != 200) {
                throw new Error(response.statusText);
            }
            loadUserInfo(user_info, response.data.id, response.data.name, response.data.auth_token);
        })
        .catch((error: Error) => {
            console.error("Error fetching user info:", error);
            throw error;
        });
}

export function passwordEncrypter(password: string) {
    console.error("password encrypter is not implemented"); //! TODO
    return password;
}

export function isLogined(user_info: UserInfo) {
    return user_info.id !== undefined && user_info.name !== undefined && user_info.auth_token !== undefined;
}
