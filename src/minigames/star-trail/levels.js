// All positions use the same 360-pixel-high world. No external assets are loaded.
export const ARMOR_TIERS = [
  { name: "旅人护甲", capacity: 2, cost: 0 },
  { name: "巡星护甲", capacity: 4, cost: 10 },
  { name: "辉光护甲", capacity: 6, cost: 18 },
];
export const LEVELS = [
  {
    name: "青苔海岸",
    subtitle: "沿着星光，找回第一盏灯",
    bossName: "苔冠守卫",
    theme: "coast",
    width: 3220,
    sky: ["#173f64", "#66c9c7"],
    land: ["#b3ed95", "#42877e", "#254e61"],
    accent: "#f9d78c",
    ground: [
      [0, 670],
      [770, 1380],
      [1470, 2040],
      [2140, 3220],
    ],
    platforms: [
      [300, 240, 100],
      [490, 205, 90],
      [625, 248, 165],
      [990, 235, 130],
      [1260, 240, 150],
      [1480, 205, 110],
      [1770, 242, 120],
      [1990, 244, 190],
      [2310, 239, 125],
    ],
    enemies: [
      [440, "slime"],
      [920, "slime"],
      [1180, "drone"],
      [1620, "slime"],
      [1860, "slime"],
      [2320, "drone"],
    ],
    checkpoint: 1670,
    arena: 2530,
    bossHP: 24,
    springs: [245, 1650],
    crates: [
      [205, "shield"],
      [505, "rapid", true],
      [1130, "magnet"],
      [1840, "heal"],
      [2390, "rapid"],
    ],
  },
  {
    name: "落日机坊",
    subtitle: "穿过齿轮与余烬",
    bossName: "炉心机甲",
    theme: "factory",
    width: 3400,
    sky: ["#452655", "#f5a176"],
    land: ["#ffc185", "#9a586a", "#493451"],
    accent: "#ffce8a",
    ground: [
      [0, 570],
      [670, 1220],
      [1320, 1950],
      [2040, 2460],
      [2550, 3400],
    ],
    platforms: [
      [280, 238, 110],
      [510, 242, 185],
      [850, 236, 115],
      [1060, 186, 105],
      [1190, 245, 150],
      [1480, 235, 125],
      [1700, 191, 115],
      [1900, 244, 175],
      [2260, 239, 125],
      [2400, 245, 180],
    ],
    enemies: [
      [430, "slime"],
      [800, "turret"],
      [1120, "drone"],
      [1430, "slime"],
      [1740, "turret"],
      [2180, "drone"],
      [2370, "slime"],
    ],
    checkpoint: 1570,
    arena: 2690,
    bossHP: 32,
    springs: [210, 1430, 2380],
    crates: [
      [180, "shield"],
      [925, "rapid", true],
      [1500, "magnet"],
      [2150, "heal"],
      [2610, "rapid"],
    ],
  },
  {
    name: "星月高塔",
    subtitle: "为沉睡的天空点亮最后一盏灯",
    bossName: "蚀星魔像",
    theme: "night",
    width: 3560,
    sky: ["#161d45", "#776bac"],
    land: ["#bcbef9", "#675b98", "#342b5e"],
    accent: "#91f5ec",
    ground: [
      [0, 620],
      [720, 1290],
      [1390, 2020],
      [2120, 2560],
      [2660, 3560],
    ],
    platforms: [
      [320, 242, 120],
      [580, 244, 165],
      [900, 237, 120],
      [1170, 240, 145],
      [1300, 189, 140],
      [1560, 244, 115],
      [1780, 192, 125],
      [1970, 245, 185],
      [2350, 242, 135],
      [2500, 245, 185],
    ],
    enemies: [
      [470, "turret"],
      [860, "drone"],
      [1120, "slime"],
      [1550, "turret"],
      [1830, "drone"],
      [2260, "slime"],
      [2440, "turret"],
    ],
    checkpoint: 1630,
    arena: 2810,
    bossHP: 40,
    springs: [250, 1490, 2260],
    crates: [
      [180, "shield"],
      [950, "rapid", true],
      [1520, "magnet"],
      [2240, "heal"],
      [2720, "shield"],
    ],
  },
];

export function makeLevel(index) {
  const source = LEVELS[index];
  const solids = [
    ...source.ground.map(([x, end]) => ({ x, y: 300, w: end - x, h: 80, floor: true })),
    ...source.platforms.map(([x, y, w]) => ({ x, y, w, h: 14, floor: false })),
  ];
  const coins = [];
  const surfaceAt = (x, w) =>
    Math.min(
      ...solids
        .filter((solid) => solid.x <= x && solid.x + solid.w >= x + w)
        .map((solid) => solid.y)
    );
  source.platforms.forEach(([x, y, w], platform) => {
    for (let i = 0; i < 3; i++)
      coins.push({ id: `p${platform}-${i}`, x: x + (w * (i + 1)) / 4, y: y - 22, w: 10, h: 12 });
  });
  source.ground.forEach(([x, end], segment) => {
    for (let at = x + 180; at < Math.min(end - 70, source.arena - 50); at += 190)
      coins.push({ id: `g${segment}-${at}`, x: at, y: 269, w: 10, h: 12 });
  });
  return {
    ...source,
    solids,
    coins,
    heals: [
      { id: "heal-a", x: 1070, y: surfaceAt(1070, 14) - 20, w: 14, h: 14 },
      {
        id: "heal-b",
        x: source.checkpoint + 120,
        y: surfaceAt(source.checkpoint + 120, 14) - 20,
        w: 14,
        h: 14,
      },
    ],
    springs: source.springs.map((x) => ({
      x,
      y: surfaceAt(x, 26) - 12,
      w: 26,
      h: 12,
      cooldown: 0,
    })),
    crates: source.crates.map(([x, reward, floating], id) => ({
      id: `crate-${id}`,
      reward,
      x,
      y: surfaceAt(x, 24) - (floating ? 64 : 24),
      w: 24,
      h: 24,
      floor: true,
      crate: true,
      opened: false,
    })),
    powers: [
      {
        id: "arena-shield",
        kind: "shield",
        x: source.arena - 105,
        y: surfaceAt(source.arena - 105, 16) - 20,
        w: 16,
        h: 16,
      },
    ],
    shop: {
      x: source.checkpoint - 60,
      y: surfaceAt(source.checkpoint - 60, 62) - 50,
      w: 62,
      h: 50,
    },
    enemies: source.enemies.map(([x, kind], id) => ({
      id,
      kind,
      x,
      y: kind === "drone" ? 212 : 278,
      origin: x,
      w: 23,
      h: 22,
      vx: 0,
      vy: 0,
      face: -1,
      hp: kind === "turret" ? 3 : 2,
      cooldown: 0.8 + id * 0.12,
      time: id,
      grounded: false,
    })),
  };
}
