export enum Position {
    Left,
    Right
}

export interface IBubble {
    position: Position;
    content: string;
    image: string;
}