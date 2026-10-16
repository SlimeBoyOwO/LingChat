/** 游戏舞台与配置预览共用静态立绘取景和布局规则。 */
export function avatarObjectFit(aspectRatio: number): string {
  if (aspectRatio >= 1.0) return "contain";
  const percent = Math.max(80, 100 - (1.0 - aspectRatio) * 40);
  return `auto ${Math.round(percent)}%`;
}

export function standardAvatarStyle(
  role: { scale: number; offsetX: number; offsetY: number },
  viewport: { width: number; height: number },
  leftPercent = 50,
  castScale = 1,
  castOffsetY = 0,
): Record<string, string> {
  const ratio = viewport.width / viewport.height;
  const compensation =
    ratio < 1
      ? Math.round((viewport.height * Math.min(20, (1 - ratio) * 40)) / 100)
      : ratio >= 2
        ? Math.round((viewport.height * Math.min(10, (ratio - 2) * 20)) / 100)
        : 0;
  const scale = (role.scale ?? 1) * castScale;
  const top = role.offsetY - compensation;
  // 投屏只夹紧其自身向下偏移，保留角色 offsetY 的既有语义。
  const downLimit = viewport.height * (1 - scale) - top;
  const castY = castOffsetY > 0 ? Math.min(castOffsetY, Math.max(0, downLimit)) : castOffsetY;
  return {
    left: `calc(${leftPercent}% + ${role.offsetX || 0}px)`,
    top: `${top + castY}px`,
    transform: `translateX(-50%) scale(${scale})`,
  };
}
