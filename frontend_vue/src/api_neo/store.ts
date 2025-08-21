import { UIStatus, createUIStatusStatic } from "./services/UIStatus.ts";
import { UserInfo, UserData, createUserInfoStatic, createUserDataStatic } from "./services/UserInfo.ts";
import { Settings, Defaults, createSettingsStatic, createDefaultsStatic } from "./services/Settings.ts";

const uiStatus: UIStatus = await createUIStatusStatic();
const userInfo: UserInfo = await createUserInfoStatic();
const userData: UserData = await createUserDataStatic();
const settings: Settings = await createSettingsStatic();
const defaults: Defaults = await createDefaultsStatic();
