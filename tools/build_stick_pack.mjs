// Generates packs/stick/pack.json: line-drawn "stick" props and backgrounds,
// plus the extra poses (worry, hands on hips, meditate, slump, kick, confused,
// thumbs up). The pack is built into the app next to the core pack.
//
//   node tools/build_stick_pack.mjs
//
// All art here is original and procedural (CC0).
import { writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const INK = "#1f2233";
const PAPER = "#ffffff";

// ---------------------------------------------------------------- helpers
const line = (x1, y1, x2, y2, sw = 6, stroke = INK) => ({ t: "line", x1, y1, x2, y2, stroke, sw });
const pathS = (d, sw = 6, stroke = INK, fill) => ({ t: "path", d, stroke, sw, ...(fill ? { fill } : {}) });
const rect = (x, y, w, h, fill = PAPER, sw = 6, r = 0) => ({ t: "rect", x, y, w, h, r, fill, stroke: INK, sw });
const circ = (cx, cy, r, fill = PAPER, sw = 6) => ({ t: "circle", cx, cy, r, fill, stroke: INK, sw });
const poly = (points, fill = PAPER, sw = 6) => ({ t: "poly", points, fill, stroke: INK, sw });

/** A hand-drawn looking scribble crown: overlapping bumps around a centre. */
function crown(cx, cy, rx, ry, bumps, fill) {
  let d = "";
  for (let i = 0; i <= bumps; i++) {
    const a = (i / bumps) * Math.PI * 2;
    const x = cx + Math.cos(a) * rx;
    const y = cy + Math.sin(a) * ry;
    if (i === 0) d += `M${x.toFixed(1)} ${y.toFixed(1)} `;
    else {
      const am = ((i - 0.5) / bumps) * Math.PI * 2;
      const qx = cx + Math.cos(am) * rx * 1.28;
      const qy = cy + Math.sin(am) * ry * 1.28;
      d += `Q${qx.toFixed(1)} ${qy.toFixed(1)} ${x.toFixed(1)} ${y.toFixed(1)} `;
    }
  }
  return pathS(d + "Z", 6, INK, fill);
}

/** A puffy cloud outline with a flat base, centred on (cx, cy), `k` = scale. */
function cloud(cx, cy, k, fill = PAPER, sw = 6) {
  const pts = [[-100, 20], [-118, -18], [-80, -40], [-64, -82], [-14, -82], [8, -112], [58, -96], [76, -58], [118, -46], [124, 0], [104, 20]];
  const P = ([x, y]) => `${(cx + x * k).toFixed(1)} ${(cy + y * k).toFixed(1)}`;
  let d = `M${P(pts[0])} `;
  for (let i = 1; i < pts.length; i++) {
    const [x0, y0] = pts[i - 1], [x1, y1] = pts[i];
    // bulge each segment outwards from the centre
    const mx = (x0 + x1) / 2, my = (y0 + y1) / 2, len = Math.hypot(mx, my + 30) || 1;
    d += `Q${P([mx + (mx / len) * 26, my + ((my + 30) / len) * 26])} ${P(pts[i])} `;
  }
  return pathS(d + "Z", sw, INK, fill);
}

// ---------------------------------------------------------------- props
const props = [
  {
    id: "stick-tree", name: "Sketch tree", category: "Stick", colors: ["#8fd694"],
    shapes: [line(0, 0, 0, -170, 10), line(0, -110, -38, -150, 7), line(0, -130, 34, -168, 7), crown(0, -235, 92, 78, 9, "$0")],
  },
  {
    id: "stick-house", name: "Sketch house", category: "Stick", colors: ["#ffd166", "#ef6f6c"],
    shapes: [
      rect(-140, -210, 280, 210, "$0"),
      poly([[-170, -205], [0, -330], [170, -205]], "$1"),
      rect(-30, -110, 60, 110, PAPER),
      circ(18, -55, 4, INK, 1),
      rect(-115, -170, 60, 50, "#bfe3ff"), line(-85, -170, -85, -120, 4), line(-115, -145, -55, -145, 4),
      rect(55, -170, 60, 50, "#bfe3ff"), line(85, -170, 85, -120, 4), line(55, -145, 115, -145, 4),
    ],
  },
  {
    id: "stick-sun", name: "Sketch sun", category: "Stick", colors: ["#ffd166"],
    shapes: [
      ...Array.from({ length: 10 }, (_, i) => {
        const a = (i / 10) * Math.PI * 2;
        return line(Math.cos(a) * 70, -110 + Math.sin(a) * 70, Math.cos(a) * 100, -110 + Math.sin(a) * 100, 6);
      }),
      circ(0, -110, 55, "$0"),
    ],
  },
  { id: "stick-cloud", name: "Sketch cloud", category: "Stick", colors: [PAPER], shapes: [cloud(0, -30, 1, "$0")] },
  {
    id: "stick-table", name: "Sketch table", category: "Stick", colors: ["#e9c891"],
    shapes: [rect(-120, -118, 240, 18, "$0", 6, 4), line(-100, -100, -100, 0, 7), line(100, -100, 100, 0, 7), line(-100, -40, 100, -40, 4)],
  },
  {
    id: "stick-chair", name: "Sketch chair", category: "Stick", colors: ["#e9c891"],
    shapes: [rect(-44, -210, 18, 120, "$0", 6, 3), line(-38, -90, -38, 0, 7), line(40, -90, 40, 0, 7), rect(-48, -98, 96, 14, "$0", 6, 3)],
  },
  {
    id: "stick-flag", name: "Sketch flag", category: "Stick", colors: ["#ef6f6c"],
    shapes: [line(0, 0, 0, -260, 7), pathS("M0 -255 Q 45 -275 90 -250 Q 125 -232 150 -248 L 150 -180 Q 120 -164 90 -182 Q 45 -205 0 -185 Z", 5, INK, "$0"), circ(0, -264, 7, INK, 1)],
  },
  {
    id: "stick-ball", name: "Sketch ball", category: "Stick", colors: ["#ffffff", "#ef6f6c"],
    shapes: [circ(0, -36, 36, "$0"), pathS("M-34 -46 Q 0 -30 34 -46", 4), pathS("M-30 -18 Q 0 -2 30 -18", 4, INK), pathS("M-2 -72 Q -16 -36 -2 0", 4)],
  },
  {
    id: "stick-sign", name: "Sketch signpost", category: "Stick", colors: ["#e9c891"],
    shapes: [line(0, 0, 0, -220, 8), poly([[-90, -230], [80, -230], [110, -200], [80, -170], [-90, -170]], "$0", 6)],
  },
  {
    id: "stick-car", name: "Sketch car", category: "Stick", colors: ["#4ea8de"],
    shapes: [
      pathS("M-190 -40 L -190 -95 L -110 -100 L -70 -160 L 80 -160 L 130 -100 L 190 -90 L 190 -40 Z", 6, INK, "$0"),
      poly([[-55, -150], [-15, -150], [-15, -102], [-90, -102]], "#bfe3ff", 5),
      poly([[0, -150], [70, -150], [110, -102], [0, -102]], "#bfe3ff", 5),
      circ(-110, -36, 34, PAPER), circ(-110, -36, 10, INK, 1), circ(115, -36, 34, PAPER), circ(115, -36, 10, INK, 1),
    ],
  },
  {
    id: "stick-flower", name: "Sketch flower", category: "Stick", colors: ["#f78fb3", "#ffd166"],
    shapes: [
      pathS("M0 0 Q -8 -50 0 -100", 6), pathS("M0 -40 Q 22 -60 34 -52 Q 20 -36 0 -40 Z", 4, INK, "#8fd694"),
      ...[0, 72, 144, 216, 288].map((deg) => {
        const a = (deg * Math.PI) / 180;
        return circ(Math.cos(a) * 18, -118 + Math.sin(a) * 18, 13, "$0", 4);
      }),
      circ(0, -118, 11, "$1", 4),
    ],
  },
  {
    id: "stick-speech", name: "Sketch bubble", category: "Stick", colors: [PAPER],
    shapes: [pathS("M-120 -200 Q -120 -260 -60 -260 L 60 -260 Q 120 -260 120 -200 Q 120 -140 60 -140 L -20 -140 L -60 -100 L -50 -140 L -60 -140 Q -120 -140 -120 -200 Z", 6, INK, "$0")],
  },
];

// ---------------------------------------------------------------- backgrounds (1280×720, origin top-left)
const backgrounds = [
  {
    id: "stick-paper", name: "Notebook paper", colors: ["#fffdf5", "#9cc3ff"],
    shapes: [
      { t: "rect", x: 0, y: 0, w: 1280, h: 720, r: 0, fill: "$0" },
      ...Array.from({ length: 17 }, (_, i) => ({ t: "line", x1: 0, y1: 70 + i * 40, x2: 1280, y2: 70 + i * 40, stroke: "$1", sw: 2, opacity: 0.6 })),
      { t: "line", x1: 120, y1: 0, x2: 120, y2: 720, stroke: "#f28b82", sw: 3 },
      ...[140, 360, 580].map((y) => ({ t: "circle", cx: 50, cy: y, r: 14, fill: "#e8e4da" })),
    ],
  },
  {
    id: "stick-whiteboard", name: "Whiteboard", colors: ["#fbfbfd", "#b9c0cc"],
    shapes: [
      { t: "rect", x: 0, y: 0, w: 1280, h: 720, r: 0, fill: "#e9ecf2" },
      { t: "rect", x: 40, y: 30, w: 1200, h: 610, r: 10, fill: "$0", stroke: "$1", sw: 14 },
      { t: "rect", x: 300, y: 640, w: 680, h: 22, r: 6, fill: "$1" },
      { t: "rect", x: 360, y: 626, w: 70, h: 16, r: 4, fill: "#ef6f6c" },
      { t: "rect", x: 450, y: 626, w: 70, h: 16, r: 4, fill: "#4ea8de" },
      { t: "line", x1: 0, y1: 700, x2: 1280, y2: 700, stroke: INK, sw: 4, opacity: 0.4 },
    ],
  },
  {
    id: "stick-hills", name: "Sketch hills", colors: ["#ffffff", "#ffd166"],
    shapes: [
      { t: "rect", x: 0, y: 0, w: 1280, h: 720, r: 0, fill: "$0" },
      ...Array.from({ length: 8 }, (_, i) => {
        const a = (i / 8) * Math.PI * 2;
        return { t: "line", x1: 1100 + Math.cos(a) * 62, y1: 120 + Math.sin(a) * 62, x2: 1100 + Math.cos(a) * 86, y2: 120 + Math.sin(a) * 86, stroke: INK, sw: 5 };
      }),
      { t: "circle", cx: 1100, cy: 120, r: 46, fill: "$1", stroke: INK, sw: 5 },
      cloud(240, 130, 0.75, PAPER, 5),
      cloud(640, 95, 0.55, PAPER, 5),
      pathS("M0 560 Q 220 440 460 540 Q 700 420 980 530 Q 1140 480 1280 510", 5),
      pathS("M0 640 L 1280 640", 6),
      ...[150, 330, 820, 1010].map((x) => pathS(`M${x} 640 l 6 -16 l 6 16 l 6 -12`, 3)),
    ],
  },
  {
    id: "stick-room", name: "Sketch room", colors: ["#ffffff", "#bfe3ff"],
    shapes: [
      { t: "rect", x: 0, y: 0, w: 1280, h: 720, r: 0, fill: "$0" },
      pathS("M0 640 L 1280 640", 6),
      pathS("M0 690 L 1280 690", 2, INK),
      { t: "rect", x: 140, y: 130, w: 250, h: 190, r: 0, fill: "$1", stroke: INK, sw: 6 },
      pathS("M265 130 L 265 320 M140 225 L 390 225", 5),
      { t: "rect", x: 960, y: 300, w: 170, h: 340, r: 0, fill: PAPER, stroke: INK, sw: 6 },
      { t: "circle", cx: 1105, cy: 475, r: 7, fill: INK },
      { t: "rect", x: 560, y: 120, w: 150, h: 110, r: 0, fill: PAPER, stroke: INK, sw: 5 },
      pathS("M575 210 L 615 160 L 645 195 L 665 175 L 695 210", 4),
    ],
  },
  {
    id: "stick-stage", name: "Blank stage", colors: ["#ffffff"],
    shapes: [{ t: "rect", x: 0, y: 0, w: 1280, h: 720, r: 0, fill: "$0" }, pathS("M60 640 L 1220 640", 5)],
  },
];

// ---------------------------------------------------------------- actions
// Angles: 0 = limb hanging down, + raises it outward on its own side, − swings
// it across the body. Elbows/knees are relative to the upper limb.
const rest = { ls: 8, le: 6, rs: 8, re: 6 };
const actions = [
  {
    id: "worry", name: "Worry", duration: 1.6,
    keys: [
      { t: 0, pose: { ls: -28, le: -118, rs: -28, re: -118, head: -4, bob: 4, look: -0.5 } },
      { t: 0.5, pose: { ls: -26, le: -122, rs: -26, re: -122, head: 4, bob: 5, look: 0.5 } },
    ],
  },
  {
    id: "hips", name: "Hands on hips", duration: 3,
    keys: [
      { t: 0, pose: { ls: 48, le: -72, rs: 48, re: -72, lh: 6, rh: 6, head: -2 } },
      { t: 0.5, pose: { ls: 50, le: -74, rs: 50, re: -74, lh: 6, rh: 6, head: 2, bob: 1.5 } },
    ],
  },
  {
    id: "meditate", name: "Meditate", duration: 4,
    keys: [
      { t: 0, pose: { rise: -66, lh: 78, lk: -152, rh: 78, rk: -152, ls: 32, le: 28, rs: 32, re: 28, head: 0 } },
      { t: 0.5, pose: { rise: -62, lh: 78, lk: -152, rh: 78, rk: -152, ls: 33, le: 30, rs: 33, re: 30, head: 1.5, bob: -2 } },
    ],
  },
  {
    id: "slump", name: "Slump", duration: 3,
    keys: [
      { t: 0, pose: { ls: -2, le: 4, rs: -2, re: 4, head: 24, bob: 12, lean: 3, lk: -6, rk: -6, lh: 3, rh: 3 } },
      { t: 0.5, pose: { ls: -3, le: 5, rs: -3, re: 5, head: 26, bob: 14, lean: 3, lk: -6, rk: -6, lh: 3, rh: 3 } },
    ],
  },
  {
    id: "kick", name: "Kick", duration: 1.3,
    keys: [
      { t: 0, pose: { ...rest } },
      { t: 0.25, pose: { ls: 40, le: 20, rs: 30, re: 30, rh: -20, rk: 70, lean: 4, bob: 4 } },
      { t: 0.45, pose: { ls: 70, le: 10, rs: 55, re: 10, rh: 95, rk: -6, lh: -6, lean: -10 } },
      { t: 0.7, pose: { ls: 30, le: 20, rs: 25, re: 20, rh: 20, rk: 20, lean: -3 } },
      { t: 0.9, pose: { ...rest } },
    ],
  },
  {
    id: "confused", name: "Confused", duration: 2,
    keys: [
      { t: 0, pose: { ls: 8, le: 6, rs: 150, re: 14, head: 10, look: -0.4 } },
      { t: 0.5, pose: { ls: 9, le: 8, rs: 154, re: 22, head: 14, look: 0.3 } },
    ],
  },
  {
    id: "thumbs-up", name: "Thumbs up", duration: 2,
    keys: [
      { t: 0, pose: { ls: 8, le: 6, rs: 32, re: 118, thumb: 1, head: 4 } },
      { t: 0.5, pose: { ls: 9, le: 8, rs: 36, re: 122, thumb: 1, head: 6, bob: 1.5 } },
    ],
  },
];

const pack = {
  id: "stick",
  name: "Stick figures",
  version: "1.0.0",
  license: "CC0-1.0",
  props,
  backgrounds,
  actions,
};
mkdirSync(path.join(root, "packs", "stick"), { recursive: true });
writeFileSync(path.join(root, "packs", "stick", "pack.json"), JSON.stringify(pack, null, 1) + "\n");
console.log(`packs/stick/pack.json: ${props.length} props, ${backgrounds.length} backgrounds, ${actions.length} actions`);
