import axios from "axios";
import { API_URLS } from "../consts.ts";
import { ModelInfo } from "./ModelInfo.ts";
export interface CharacterAvatars {
    //urls
    happy?: string;
    sad?: string;
    // TODO
}
export interface CharacterCardCover {
    id: number;
    cover: string;
    title: string;
    description: string;
}
export class CharacterCard {
    cover: CharacterCardCover;
    player_name: string;
    player_subtitle: string;
    ai_name: string;
    ai_subtitle: string;
    ai_model: ModelInfo;
    ai_avatars: CharacterAvatars;
    constructor(
        cover: CharacterCardCover,
        player_name: string,
        player_subtitle: string,
        ai_name: string,
        ai_subtitle: string,
        ai_model: ModelInfo,
        ai_avatars: CharacterAvatars
    ) {
        this.cover = cover;
        this.player_name = player_name;
        this.player_subtitle = player_subtitle;
        this.ai_name = ai_name;
        this.ai_subtitle = ai_subtitle;
        this.ai_model = ai_model;
        this.ai_avatars = ai_avatars;
    }
}
export async function getCharacterCardCover(card_id: number[]): Promise<CharacterCardCover[]> {
    return axios
        .post(API_URLS.CARD.CHARACTER.COVER, {
            id: card_id
        })
        .then(response => {
            return Array.from<CharacterCardCover>(response.data.values());
        })
        .catch((error: Error) => {
            console.error("Error fetching character card cover:", error);
            throw error;
        });
}

export async function getCharacterCardExtend(covers: CharacterCardCover[]): Promise<CharacterCard[]> {
    return axios
        .post(API_URLS.CARD.CHARACTER.EXTEND, {
            id: covers.map(cover => cover.id)
        })
        .then(response => {
            return covers.map(cover => new CharacterCard(
                cover,
                response.data[cover.id].player_name,
                response.data[cover.id].player_subtitle,
                response.data[cover.id].ai_name,
                response.data[cover.id].ai_subtitle,
                response.data[cover.id].ai_model as ModelInfo,
                response.data[cover.id].ai_avatars as CharacterAvatars
            ));
        })
        .catch((error: Error) => {
            console.error("Error fetching character card:", error);
            throw error;
        });
}

export async function getCharacterCardFull(card_id:number[]): Promise<CharacterCard[]> {
    return axios
        .post(API_URLS.CARD.CHARACTER.SINGLE, {
            id: card_id
        })
        .then(response => {
            return Array.from<CharacterCard>(response.data.values());
        })
        .catch((error: Error) => {
            console.error("Error fetching full character card:", error);
            throw error;
        });
}

export async function searchCharacterCards(card_name:string): Promise<CharacterCardCover[]> {
    return axios.post(API_URLS.CARD.CHARACTER.SEARCH, {
        name:card_name
    }).then(response => {
        return Array.from<CharacterCardCover>(response.data.values());
    })
}