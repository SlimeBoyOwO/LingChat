import { IconType } from "../../components/controls/Icon.vue";
import { CustomControl } from "./CustomControls";

type HTMLElementName = string;
export type SettingPage = {
    element: HTMLElementName | CustomControl;
    bindings: {
        title: string;
        show: boolean;
        [property: string]: unknown;
    }
};

export type SettingNode1 = {
    [title: string]: { icon: IconType; content: SettingPage[] | SettingNode2 };
};

export type SettingNode2 = {
    [title: string]: { icon: IconType; content: SettingPage[] };
};
