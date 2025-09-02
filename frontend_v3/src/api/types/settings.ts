type HTMLElementName = string;
export type SettingPage = {
    element: HTMLElementName;
    type?: string;
    title: string;
    class: string;
    value: string | number | boolean;
    action?: (event: Event) => void;
    show: boolean;
};

export type SettingNode1 = {
    [title: string]: { icon: string; content: SettingPage[] | SettingNode2 };
};

export type SettingNode2 = {
    [title: string]: { icon: string; content: SettingPage[] };
};
