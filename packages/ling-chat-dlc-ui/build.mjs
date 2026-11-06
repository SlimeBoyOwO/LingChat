import { readFileSync } from "node:fs";
const document = () =>
  readFileSync(new URL("./public/script-glitch.html", import.meta.url));
export function extensionAssets() {
  return {
    name: "lingchat-extension-assets",
    configureServer(server) {
      server.middlewares.use("/script-glitch.html", (_req, res) => {
        res.setHeader("Content-Type", "text/html; charset=utf-8");
        res.end(document());
      });
    },
    generateBundle() {
      this.emitFile({
        type: "asset",
        fileName: "script-glitch.html",
        source: document(),
      });
    },
  };
}
