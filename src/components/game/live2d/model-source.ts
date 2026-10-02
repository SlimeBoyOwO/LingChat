import type { Live2dMotionBinding, Live2dVariantAssets } from "@/types/live2d";

export interface Live2dModelReferences {
  Moc?: string;
  Textures?: string[];
  Physics?: string;
  Pose?: string;
  UserData?: string;
  DisplayInfo?: string;
  Expressions?: Array<{ File?: string; [key: string]: unknown }>;
  Motions?: Record<string, Array<{ File?: string; Sound?: string; [key: string]: unknown }>>;
  [key: string]: unknown;
}

export interface Live2dModelSource {
  FileReferences?: Live2dModelReferences;
  url?: string;
  [key: string]: unknown;
}

export const RUNTIME_IDLE_GROUP = "__LingChatConfiguredIdle";

export function configureRuntimeIdle(
  source: Live2dModelSource,
  idle: Live2dMotionBinding | null | undefined,
): Live2dMotionBinding | null {
  if (!idle) return null;
  const motions = source.FileReferences?.Motions;
  const definition = motions?.[idle.group]?.[idle.index];
  if (!motions || !definition) {
    throw new Error(`Configured Live2D idle motion does not exist: ${idle.group}[${idle.index}]`);
  }
  motions[RUNTIME_IDLE_GROUP] = [{ ...definition }];
  return { group: RUNTIME_IDLE_GROUP, index: 0, loop: idle.loop ?? true };
}

/**
 * 把可用资源表补进 model3.json 的 `FileReferences`——只补缺，绝不替换或重排。
 *
 * VTube Studio 式导出的 model3.json 里没有 `Expressions`/`Motions` 段（资源散在模型
 * 目录下），而引擎的 `ExpressionManager` 只从这两段建表，名字不在表里时
 * `setExpression` 会静默返回 `false`（不抛错）。补进去之后，用户在设置界面绑定的
 * 表情名才真的能生效。
 *
 * 两个「不」都在保护用户已有配置：
 * - 不替换 `Expressions`：名字是绑定的键，只追加缺失的名字，已有顺序不动。
 * - 不碰已存在的动作组：`motions[情绪] = {group, index}` 按 index 定位，往已有组里
 *   插入或重排文件会让这个情绪的所有绑定整体错位。
 *
 * 必须在 `rewriteModelReferences` 之前调用，否则新补进来的相对路径不会被转成文件 URL。
 */
export function mergeVariantAssets(
  source: Live2dModelSource,
  assets: Live2dVariantAssets | null | undefined,
): void {
  if (!assets) return;
  const references = source.FileReferences;
  if (!references) return;

  const existingExpressions = references.Expressions ?? [];
  const knownNames = new Set(
    existingExpressions
      .map((expression) => expression?.Name)
      .filter((name): name is string => typeof name === "string"),
  );
  const addedExpressions = Object.entries(assets.expressions)
    .filter(([name]) => !knownNames.has(name))
    .map(([Name, File]) => ({ Name, File }));
  // 引擎用 `if (settings.expressions)` 判断要不要建 ExpressionManager，
  // 补不出东西时别凭空造一个空的 Expressions 数组
  if (addedExpressions.length > 0) {
    references.Expressions = [...existingExpressions, ...addedExpressions];
  }

  const existingMotions = references.Motions ?? {};
  const addedMotions: Record<string, Array<{ File: string }>> = {};
  for (const [group, files] of Object.entries(assets.motions)) {
    if (files.length === 0) continue;
    if (Object.prototype.hasOwnProperty.call(existingMotions, group)) continue;
    addedMotions[group] = files.map((File) => ({ File }));
  }
  if (Object.keys(addedMotions).length > 0) {
    references.Motions = { ...existingMotions, ...addedMotions };
  }
}

const URL_SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;

export function resolveModelReference(modelFile: string, reference: string): string {
  if (
    URL_SCHEME.test(reference) ||
    reference.startsWith("/") ||
    /^[a-zA-Z]:[\\/]/.test(reference)
  ) {
    throw new Error(`Live2D resource reference must be relative: ${reference}`);
  }

  const segments = modelFile.split("\\").join("/").split("/");
  segments.pop();
  for (const segment of reference.split("\\").join("/").split("/")) {
    if (!segment || segment === ".") continue;
    if (segment === "..") {
      if (!segments.length) {
        throw new Error(`Live2D resource escapes the role directory: ${reference}`);
      }
      segments.pop();
    } else {
      segments.push(segment);
    }
  }
  return segments.join("/");
}

export async function rewriteModelReferences(
  source: Live2dModelSource,
  modelFile: string,
  resolveFileUrl: (roleRelativePath: string) => Promise<string>,
): Promise<Live2dModelSource> {
  const references = source.FileReferences;
  if (!references) throw new Error("Live2D model3 is missing FileReferences");

  const rewrite = (reference: string) =>
    resolveFileUrl(resolveModelReference(modelFile, reference));
  const rewrites: Promise<void>[] = [];

  for (const key of ["Moc", "Physics", "Pose", "UserData", "DisplayInfo"] as const) {
    const reference = references[key];
    if (typeof reference === "string") {
      rewrites.push(
        rewrite(reference).then((url) => {
          references[key] = url;
        }),
      );
    }
  }

  references.Textures?.forEach((reference, index) => {
    rewrites.push(
      rewrite(reference).then((url) => {
        references.Textures![index] = url;
      }),
    );
  });

  references.Expressions?.forEach((expression) => {
    if (typeof expression.File === "string") {
      rewrites.push(
        rewrite(expression.File).then((url) => {
          expression.File = url;
        }),
      );
    }
  });

  for (const motions of Object.values(references.Motions ?? {})) {
    for (const motion of motions) {
      if (typeof motion.File === "string") {
        rewrites.push(
          rewrite(motion.File).then((url) => {
            motion.File = url;
          }),
        );
      }
      if (typeof motion.Sound === "string" && motion.Sound.length > 0) {
        rewrites.push(
          rewrite(motion.Sound).then((url) => {
            motion.Sound = url;
          }),
        );
      }
    }
  }

  await Promise.all(rewrites);
  return source;
}
