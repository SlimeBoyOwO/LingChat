import effectManifest from "../../../../../shared/script-effects.json";

/** 前后端共用的剧本背景/恐怖特效清单，唯一来源为 shared/script-effects.json。 */
export interface ParticleEffect {
  key: string;
  label: string;
  layer: "background" | "horror";
  horror: boolean;
}

export const PARTICLE_EFFECTS: readonly ParticleEffect[] = effectManifest as ParticleEffect[];
export const BACKGROUND_EFFECTS = PARTICLE_EFFECTS.filter(
  (effect) => effect.layer === "background"
);
export const HORROR_EFFECT_KEYS = PARTICLE_EFFECTS.filter((effect) => effect.horror).map(
  (effect) => effect.key
);

/** 编辑器下拉使用完整清单，避免前端用五项普通粒子覆盖 Rust 的恐怖特效选项。 */
export const particleEffectOptions = (): { value: string; label: string }[] => [
  { value: "None", label: "无特效" },
  ...PARTICLE_EFFECTS.map((effect) => ({ value: effect.key, label: effect.label })),
];

/** 大小写不敏感地映射到共享清单中的规范 key；组合特效逐段规范化。 */
export const canonicalEffectKey = (value: string): string | null => {
  const raw = value.trim();
  if (!raw || raw.toLowerCase() === "none") return "None";
  const canonical: string[] = [];
  for (const part of raw.split("+").map((item) => item.trim())) {
    const match = PARTICLE_EFFECTS.find(
      (effect) => effect.key.toLowerCase() === part.toLowerCase()
    );
    if (!match) return null;
    canonical.push(match.key);
  }
  return canonical.join("+");
};
