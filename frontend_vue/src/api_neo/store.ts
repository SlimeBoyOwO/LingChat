import { UIStatus, createUIStatusStatic } from "./services/UIStatus.ts";
import { UserInfo, UserData, createUserInfoStatic, createUserDataStatic } from "./services/UserInfo.ts";
import { Settings, Defaults, createSettingsStatic, createDefaultsStatic } from "./services/Settings.ts";

const uiStatus: UIStatus = createUIStatusStatic();
const userInfo: UserInfo = createUserInfoStatic();
const userData: UserData = createUserDataStatic();
const settings: Settings = createSettingsStatic();
const defaults: Defaults = createDefaultsStatic();
