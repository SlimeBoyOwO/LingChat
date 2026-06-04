# tauri-plugin-screenshots

> This plugin only works on tauri v2, if you need the v1 plugin, feel free to submit a PR!

Get screenshots of windows and monitors.

https://github.com/user-attachments/assets/3ff503d8-46e6-47d5-8f0b-d4da181514d7

## Install

```shell
cargo add tauri-plugin-screenshots
```

You can install the JavaScript Guest bindings using your preferred JavaScript package manager:

```shell
pnpm add tauri-plugin-screenshots-api
```

## Usage

`src-tauri/src/lib.rs`

```diff
pub fn run() {
    tauri::Builder::default()
+       .plugin(tauri_plugin_screenshots::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`src-tauri/capabilities/default.json`

```diff
{
    ...
    "permissions": [
        ...
+       "screenshots:default"
    ]
}
```

Afterwards all the plugin's APIs are available through the JavaScript guest bindings:

```ts
import {
  getScreenshotableWindows,
  getWindowScreenshot,
} from "tauri-plugin-screenshots-api";

const windows = await getScreenshotableWindows();
const path = await getWindowScreenshot(windows[0].id);
console.log(path); // xx/tauri-plugin-screenshots/window-{id}.png
```

## Methods

| Method                      | Description                                             |
| --------------------------- | ------------------------------------------------------- |
| `getScreenshotableWindows`  | Get all windows that can take screenshots.              |
| `getScreenshotableMonitors` | Get all monitors that can take screenshots.             |
| `getWindowScreenshot`       | Get a screenshot of the window with the specified id.   |
| `getMonitorScreenshot`      | Get a screenshot of the monitors with the specified id. |
| `removeWindowScreenshot`    | Remove locally stored window screenshots.               |
| `removeMonitorScreenshot`   | Remove locally stored monitor screenshots.              |
| `clearScreenshots`          | Remove all locally stored screenshots.                  |

## Example

```shell
git clone https://github.com/ayangweb/tauri-plugin-screenshots.git
```

```shell
pnpm install

pnpm build

cd examples/tauri-app

pnpm install

pnpm tauri dev
```

## Thanks

- Use [xcap](https://github.com/nashaofu/xcap) to get window and monitor screenshots.

## Who's Using It?

- [Coco AI](https://github.com/infinilabs/coco-app) - Search, Connect, Collaborate, Your Personal AI Search and Assistant, all in one space.
