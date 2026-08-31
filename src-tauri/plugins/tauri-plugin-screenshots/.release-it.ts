import type { Config } from "release-it";

export default {
  git: {
    commitMessage: "tauri-plugin-screenshots-api v${version}",
    tagName: "v${version}",
  },
  npm: {
    publish: true,
  },
} satisfies Config;
