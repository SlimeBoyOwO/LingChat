import axios from "axios";
import { ModelInfo } from "./ModelInfo.ts";
export interface CharacterAvatars {
    //urls
    happy: string;
}

export class CharacterCard {
    player_name: string;
    player_subtitle: string;
    ai_name: string;
    ai_subtitle: string;
    ai_model: ModelInfo;
    ai_avatars: CharacterAvatars;
    constructor(
        player_name: string,
        player_subtitle: string,
        ai_name: string,
        ai_subtitle: string,
        ai_model: ModelInfo,
        ai_avatars: CharacterAvatars
    ) {
        this.player_name = player_name;
        this.player_subtitle = player_subtitle;
        this.ai_name = ai_name;
        this.ai_subtitle = ai_subtitle;
        this.ai_model = ai_model;
        this.ai_avatars = ai_avatars;
    }
}
export async function getCharacterCard(): Promise<CharacterCard> {
    return axios
        .get("/api/character_card")
        .then(response => {
            if (response.status != 200) {
                throw new Error(response.statusText);
            }
            return new CharacterCard(
                response.data.player_name,
                response.data.player_subtitle,
                response.data.ai_name,
                response.data.ai_subtitle,
                response.data.ai_model as ModelInfo,
                response.data.ai_avatars as CharacterAvatars
            );
        })
        .catch((error: Error) => {
            console.error("Error fetching character card:", error);
            throw error;
        });
}
