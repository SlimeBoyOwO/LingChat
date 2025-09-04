import * as ctl from "../../components/controls";

export type CustomControl =
    | typeof ctl.IconButton
    | typeof ctl.Input
    | typeof ctl.Slider
    | typeof ctl.Text
    | typeof ctl.Toggle
    | typeof ctl.Icon;
