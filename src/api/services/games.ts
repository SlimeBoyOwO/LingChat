import { invoke } from "@tauri-apps/api/core";

export interface GameSpec {
  id: string;
  name: string;
  entry: string;
  api_version: number;
  legacy_storage_keys: string[];
}
export interface GameCatalogEntry {
  id: string;
  name: string;
  description: string;
  version: string;
  url: string;
  sha256: string;
  size: number;
  api_version: number;
}
export interface GameCatalogCache {
  source: string;
  catalog: { schema_version: number; packages: GameCatalogEntry[] };
}

/** Catalog versions are validated as SemVer by Rust; build metadata is ignored. */
export function newerGameVersion(remote: string, local: string): boolean {
  const parse = (version: string) => {
    const [core, ...suffix] = version.split("+")[0].split("-");
    return { core: core.split("."), pre: suffix.join("-") };
  };
  const a = parse(remote),
    b = parse(local);
  for (let i = 0; i < 3; i++)
    if (a.core[i] !== b.core[i]) return BigInt(a.core[i]) > BigInt(b.core[i]);
  if (!a.pre || !b.pre) return !a.pre && !!b.pre;
  const left = a.pre.split("."),
    right = b.pre.split(".");
  for (let i = 0; i < Math.max(left.length, right.length); i++) {
    if (left[i] === undefined || right[i] === undefined) return left[i] !== undefined;
    if (left[i] === right[i]) continue;
    const an = /^\d+$/.test(left[i]),
      bn = /^\d+$/.test(right[i]);
    if (an && bn) return BigInt(left[i]) > BigInt(right[i]);
    return an !== bn ? !an : left[i] > right[i];
  }
  return false;
}
export const cachedGameCatalog = () => invoke<GameCatalogCache>("game_catalog_cached");
export const syncGameCatalog = () => invoke<GameCatalogCache>("game_catalog_sync");
export const installGameDlc = (id: string) => invoke<void>("game_catalog_install", { id });
export const openDlcGame = (
  pluginId: string,
  gameId: string,
  locale: string,
  outputDevice: string,
) =>
  invoke<void>("game_open", {
    pluginId,
    gameId,
    preferences: {
      locale,
      output_device: outputDevice,
      reduced_motion: matchMedia("(prefers-reduced-motion: reduce)").matches,
    },
  });
export const seedGameSave = (pluginId: string, gameId: string, value: Record<string, string>) =>
  invoke<void>("game_seed_save", { pluginId, gameId, value });
