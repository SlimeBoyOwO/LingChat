export interface CharacterImage {
  id: string;
  url: string;
  name: string;
  attributes: {
    gender: string;
    age: number;
    // 其他属性
  };
}

export interface CharacterInfo {
  ai_name: string;
  ai_subtitle: string;
  user_name: string;
  user_subtitle: string;
}

export interface Character {
  id: string;
  name: string;
  description: string;
  avatar: string;
  // 其他属性...
}

export interface CharacterSelectParams {
  user_id: string;
  character_id: string;
}
