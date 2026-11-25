// Generates packs/scenes/pack.json: extra backgrounds and props (original art, CC0).
//
//   node tools/build_scenes_pack.mjs
//
// Backgrounds are drawn on the 1280x720 design canvas with the floor at y = 560.
// Vertical 9:16 frames show the middle slice (about x 437..843), so every room
// keeps something to look at near the centre. Props have their origin at the
// base centre (y grows down, so they are drawn at negative y).

import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const out = join(dirname(fileURLToPath(import.meta.url)), "..", "packs", "scenes", "pack.json");

// ---------------------------------------------------------------- helpers
const rect = (x, y, w, h, fill, r = 0, extra = {}) => ({ t: "rect", x, y, w, h, r, fill, ...extra });
const circle = (cx, cy, r, fill, extra = {}) => ({ t: "circle", cx, cy, r, fill, ...extra });
const ellipse = (cx, cy, rx, ry, fill, extra = {}) => ({ t: "ellipse", cx, cy, rx, ry, fill, ...extra });
const poly = (points, fill, extra = {}) => ({ t: "poly", points, fill, ...extra });
const line = (x1, y1, x2, y2, stroke, sw = 3, extra = {}) => ({ t: "line", x1, y1, x2, y2, stroke, sw, ...extra });
const path = (d, fill, extra = {}) => ({ t: "path", d, fill, ...extra });
const lin = (x0, y0, x1, y1, ...stops) => ({ lin: [x0, y0, x1, y1], stops });

/** Wall, skirting and floor: the base of every room. */
function room(wall, floor, { skirting = "#ffffff", floorTo = "$1d" } = {}) {
  return [
    rect(0, 0, 1280, 560, wall),
    rect(0, 0, 1280, 260, "$0l", 0, { opacity: 0.45 }),
    rect(0, 545, 1280, 15, skirting),
    rect(0, 560, 1280, 160, lin(0, 560, 0, 720, floor, floorTo)),
  ];
}
function windowAt(x, y, w, h, sky = ["#8fd0ff", "#d8f1ff"]) {
  return [
    rect(x - 10, y - 10, w + 20, h + 20, "#ffffff", 6),
    rect(x, y, w, h, lin(0, y, 0, y + h, ...sky)),
    circle(x + w * 0.3, y + h * 0.4, 20, "#ffffff", { opacity: 0.85 }),
    circle(x + w * 0.3 + 26, y + h * 0.42, 15, "#ffffff", { opacity: 0.85 }),
    rect(x + w / 2 - 5, y, 10, h, "#ffffff"),
    rect(x, y + h / 2 - 5, w, 10, "#ffffff"),
  ];
}
function frame(x, y, w, h, inner) {
  return [rect(x - 8, y - 8, w + 16, h + 16, "#6b4630", 4), rect(x, y, w, h, inner)];
}
function planks(y0, color, n = 8) {
  const s = [];
  for (let i = 1; i < n; i++) s.push(line((1280 / n) * i - 40, y0, (1280 / n) * i + 40, 720, color, 2, { opacity: 0.25 }));
  return s;
}

// ---------------------------------------------------------------- backgrounds
const backgrounds = [];
const bg = (id, name, colors, shapes) => backgrounds.push({ id, name, colors, shapes });

bg("kitchen", "Kitchen", ["#dff1ea", "#d9c2a1", "#3fae6a"], [
  ...room("$0", "$1"),
  // Tiles behind the counter.
  ...Array.from({ length: 14 }, (_, i) => rect(i * 92, 300, 90, 160, "#ffffff", 0, { opacity: 0.35 })),
  // Upper cabinets.
  rect(60, 70, 420, 170, "$2", 8), rect(800, 70, 420, 170, "$2", 8),
  ...[80, 290, 820, 1030].map((x) => rect(x, 86, 180, 138, "$2l", 6, { opacity: 0.6 })),
  ...[250, 270, 990, 1010].map((x) => rect(x, 150, 6, 30, "#ffffff", 3)),
  // Window over the sink, in the middle.
  ...windowAt(540, 80, 200, 170),
  // Counter.
  rect(0, 440, 1280, 24, "#f4f1ea", 4), rect(0, 464, 1280, 81, "$2d"),
  ...[100, 330, 560, 790, 1020].map((x) => rect(x, 476, 200, 58, "$2", 6)),
  ...[195, 425, 655, 885, 1115].map((x) => rect(x - 18, 498, 36, 6, "#ffffff", 3)),
  // Sink tap, kettle and a fruit bowl.
  rect(630, 400, 8, 42, "#9aa3b5", 3), rect(630, 400, 34, 8, "#9aa3b5", 3),
  path("M 870 440 L 878 392 Q 900 376 922 392 L 930 440 Z", "#e84a5f"), rect(890, 372, 20, 8, "#2a2f3c", 3),
  ellipse(360, 432, 60, 16, "#ffffff"), circle(335, 420, 16, "#f2b84b"), circle(365, 414, 17, "#e84a5f"), circle(392, 422, 14, "#3fae6a"),
]);

bg("bedroom", "Bedroom", ["#d9d4f5", "#c9a27a", "#7c5cff"], [
  ...room("$0", "$1"),
  ...windowAt(160, 110, 230, 200, ["#2a2f5c", "#7c6fd0"]),
  circle(330, 160, 22, "#fff3c4"),
  // Bed with headboard in the middle.
  rect(470, 330, 340, 120, "$2d", 18), rect(450, 440, 380, 70, "$2", 12), rect(450, 470, 380, 60, "#ffffff", 12),
  rect(490, 410, 120, 50, "#ffffff", 20), rect(670, 410, 120, 50, "#ffffff", 20),
  rect(460, 520, 20, 40, "$2d"), rect(800, 520, 20, 40, "$2d"),
  // Night stand and lamp.
  rect(880, 440, 110, 105, "#a26e4a", 6), rect(895, 470, 80, 8, "#7a5236", 4),
  rect(925, 380, 20, 60, "#2a2f3c"), poly([[895, 380], [975, 380], [955, 330], [915, 330]], "#f2b84b"),
  // Poster and string lights.
  ...frame(1030, 120, 150, 200, lin(0, 120, 0, 320, "#ff9f1c", "#e84a5f")),
  path("M 0 40 Q 320 100 640 40 Q 960 100 1280 40", "#00000000", { stroke: "#2a2f3c", sw: 2 }),
  ...Array.from({ length: 16 }, (_, i) => {
    const x = 40 + i * 80;
    const y = 40 + 30 * Math.sin(((x % 640) / 640) * Math.PI);
    return circle(x, y + 8, 7, ["#ffd23f", "#ff4d6d", "#3ddc84", "#4f7cff"][i % 4]);
  }),
]);

bg("cafe", "Café", ["#f3e2c7", "#6b4630", "#2f6b4f"], [
  ...room("$0", "$1", { skirting: "$1l" }),
  ...planks(560, "#000000"),
  // Chalkboard menu in the middle.
  rect(470, 70, 340, 210, "#6b4630", 8), rect(482, 82, 316, 186, "#24352e", 4),
  ...[110, 150, 190, 230].map((y, i) => line(510, y, 690 - i * 20, y, "#f4f1ea", 4, { opacity: 0.7 })),
  ...[110, 150, 190, 230].map((y) => line(730, y, 770, y, "#ffd23f", 4, { opacity: 0.8 })),
  // Pendant lamps.
  ...[300, 980].flatMap((x) => [line(x, 0, x, 120, "#2a2f3c", 2), path(`M ${x - 40} 160 Q ${x} 100 ${x + 40} 160 Z`, "$2"), circle(x, 162, 8, "#fff3c4")]),
  // Counter with an espresso machine and cups.
  rect(0, 400, 1280, 26, "$1l", 4), rect(0, 426, 1280, 119, "$2"),
  ...[0, 1, 2, 3, 4, 5, 6].map((i) => rect(20 + i * 185, 440, 165, 90, "$2d", 6)),
  rect(170, 300, 130, 100, "#9aa3b5", 10), rect(185, 320, 100, 40, "#2a2f3c", 6), rect(225, 360, 20, 26, "#2a2f3c"),
  ...[880, 930, 980].map((x) => [rect(x, 370, 34, 30, "#ffffff", 6)]).flat(),
  // Plant on the counter.
  rect(1120, 360, 50, 40, "#c45d4f", 6), circle(1132, 340, 22, "#3fae6a"), circle(1158, 330, 24, "#2f8f55"), circle(1146, 312, 20, "#3fae6a"),
]);

bg("street", "Street", ["#bfe3ff", "#9aa3b5", "#e8a35c"], [
  rect(0, 0, 1280, 560, lin(0, 0, 0, 560, "$0", "$0l")),
  circle(1080, 110, 50, "#fff3c4"),
  // Shop fronts.
  ...[
    [0, 260, "$2"], [260, 300, "#7c5cff"], [560, 240, "#3fae6a"], [800, 260, "#e84a5f"], [1060, 220, "#4f7cff"],
  ].flatMap(([x, w, c], i) => {
    const top = [140, 90, 120, 70, 110][i];
    return [
      rect(x, top, w, 560 - top, c),
      rect(x, top, w, 18, "#ffffff", 0, { opacity: 0.35 }),
      rect(x + 20, 380, w - 40, 120, "#d8f1ff"),
      rect(x + 20, 380, w - 40, 120, "#ffffff", 0, { opacity: 0.25 }),
      path(`M ${x + 10} 340 L ${x + w - 10} 340 L ${x + w - 30} 380 L ${x + 30} 380 Z`, "#ffffff", { opacity: 0.9 }),
      ...Array.from({ length: Math.floor((w - 40) / 40) }, (_, k) => rect(x + 20 + k * 40, 340, 20, 40, c, 0, { opacity: 0.6 })),
      rect(x + 30, top + 40, w - 60, 80, "#ffffff", 4, { opacity: 0.15 }),
    ];
  }),
  // Pavement and road.
  rect(0, 500, 1280, 60, "$1l"),
  ...Array.from({ length: 16 }, (_, i) => line(i * 80, 500, i * 80, 560, "$1", 2)),
  rect(0, 560, 1280, 160, "#3d4250"),
  ...Array.from({ length: 8 }, (_, i) => rect(i * 170 + 30, 636, 90, 10, "#f4f1ea", 4)),
  // Street lamp near the middle.
  rect(700, 260, 10, 240, "#2a2f3c"), rect(690, 250, 30, 16, "#2a2f3c", 4), circle(705, 272, 10, "#fff3c4"),
]);

bg("hallway", "School hallway", ["#f3efe2", "#b8b2a3", "#4f7cff"], [
  ...room("$0", "$1", { skirting: "$1d" }),
  rect(0, 300, 1280, 245, "#dcd6c4"),
  // Lockers right across, so a vertical slice is full of them.
  ...Array.from({ length: 16 }, (_, i) => [
    rect(i * 80 + 2, 190, 76, 355, "$2", 3),
    rect(i * 80 + 12, 210, 56, 6, "$2d", 2), rect(i * 80 + 12, 222, 56, 6, "$2d", 2), rect(i * 80 + 12, 234, 56, 6, "$2d", 2),
    rect(i * 80 + 60, 360, 8, 30, "#c9ced8", 3),
  ]).flat(),
  rect(0, 60, 1280, 110, "#ffffff", 0, { opacity: 0.5 }),
  // Clock and a banner.
  circle(640, 110, 36, "#ffffff"), circle(640, 110, 30, "#2a2f3c"), line(640, 110, 640, 88, "#ffffff", 3), line(640, 110, 656, 110, "#ffffff", 3),
  rect(180, 80, 300, 60, "#e84a5f", 8), rect(800, 80, 300, 60, "#3fae6a", 8),
]);

bg("forest", "Forest", ["#cdeccf", "#5aa65e", "#2f6b4f"], [
  rect(0, 0, 1280, 560, lin(0, 0, 0, 560, "#a8dcff", "$0")),
  ...[[-40, 380, 300], [260, 330, 260], [520, 360, 340], [860, 320, 300], [1120, 370, 280]].map(([x, y, w]) => ellipse(x + w / 2, y + 60, w, 180, "$2", { opacity: 0.55 })),
  // Trees: trunks and round crowns, some in the middle.
  ...[[90, 1.1], [330, 0.85], [560, 1.2], [760, 0.95], [980, 1.15], [1200, 0.9]].flatMap(([x, k]) => [
    rect(x - 14 * k, 560 - 240 * k, 28 * k, 240 * k, "#7a5236"),
    circle(x, 560 - 270 * k, 95 * k, "$2"),
    circle(x - 55 * k, 560 - 230 * k, 70 * k, "$2d"),
    circle(x + 50 * k, 560 - 240 * k, 75 * k, "$2l"),
  ]),
  rect(0, 540, 1280, 180, lin(0, 540, 0, 720, "$1", "$1d")),
  // A dirt path winding toward the viewer.
  path("M 560 540 Q 520 610 600 660 Q 680 700 640 720 L 860 720 Q 880 690 800 650 Q 700 600 720 540 Z", "#c9a27a"),
  ...[[200, 600], [420, 690], [1000, 620], [1150, 690]].map(([x, y]) => ellipse(x, y, 40, 12, "$1l", { opacity: 0.7 })),
]);

bg("space", "Outer space", ["#0d0f2b", "#3b2a7a", "#ff6b9a"], [
  rect(0, 0, 1280, 720, lin(0, 0, 0, 720, "$0", "$1")),
  ...Array.from({ length: 90 }, (_, i) => circle((i * 397) % 1280, (i * 211) % 700, 1 + (i % 3), "#ffffff", { opacity: 0.4 + (i % 5) * 0.12 })),
  circle(980, 170, 90, lin(890, 80, 1070, 260, "$2", "#7c5cff")),
  ellipse(980, 170, 150, 26, "#00000000", { stroke: "#ffd23f", sw: 6, opacity: 0.8 }),
  circle(260, 120, 40, "#4f7cff"), circle(250, 110, 12, "#ffffff", { opacity: 0.4 }),
  // A moon surface to stand on.
  path("M 0 560 Q 320 520 640 545 Q 960 570 1280 530 L 1280 720 L 0 720 Z", "#9aa3b5"),
  ...[[180, 610, 50], [520, 660, 34], [760, 600, 60], [1100, 650, 40]].map(([x, y, r]) => ellipse(x, y, r, r * 0.35, "#6f7790")),
]);

bg("concert", "Concert stage", ["#1a1330", "#2a2f3c", "#ff4d6d"], [
  rect(0, 0, 1280, 720, "$0"),
  // Spotlight beams converging in the middle.
  ...[[120, "#ff4d6d"], [400, "#4f7cff"], [880, "#3ddc84"], [1160, "#ffd23f"]].map(([x, c]) => poly([[x - 30, 0], [x + 30, 0], [700 - (x - 640) * 0.1, 560], [580 - (x - 640) * 0.1, 560]], c, { opacity: 0.22 })),
  // Truss and lights.
  rect(0, 30, 1280, 26, "#3d4250"), ...Array.from({ length: 10 }, (_, i) => rect(i * 130 + 40, 56, 40, 34, "#2a2f3c", 6)),
  ...Array.from({ length: 10 }, (_, i) => circle(i * 130 + 60, 84, 10, ["#ff4d6d", "#4f7cff", "#3ddc84", "#ffd23f"][i % 4])),
  // Speakers and the stage.
  rect(60, 300, 140, 250, "#14161f", 8), circle(130, 380, 40, "#2a2f3c"), circle(130, 480, 50, "#2a2f3c"),
  rect(1080, 300, 140, 250, "#14161f", 8), circle(1150, 380, 40, "#2a2f3c"), circle(1150, 480, 50, "#2a2f3c"),
  rect(0, 540, 1280, 20, "$2"), rect(0, 560, 1280, 160, lin(0, 560, 0, 720, "$1", "#0a0a12")),
  ellipse(640, 600, 320, 40, "#ffffff", { opacity: 0.08 }),
]);

bg("podcast-studio", "Podcast studio", ["#2b2f45", "#5a4a3a", "#7c5cff"], [
  ...room("$0", "$1", { skirting: "#1d2030" }),
  // Acoustic foam panels.
  ...Array.from({ length: 7 }, (_, i) => Array.from({ length: 3 }, (_, j) => rect(90 + i * 160, 70 + j * 110, 130, 90, j % 2 === i % 2 ? "$2" : "$2d", 10))).flat(),
  // ON AIR sign in the middle.
  rect(560, 20, 160, 40, "#14161f", 8), rect(568, 26, 144, 28, "#e84a5f", 5),
  ...[[586, 0], [606, 1], [636, 2], [658, 3], [678, 4]].map(([x]) => rect(x, 34, 10, 12, "#ffffff", 2, { opacity: 0.9 })),
  // Warm lamp glow and a shelf.
  circle(160, 470, 120, "#ffd9a0", { opacity: 0.08 }),
  rect(960, 410, 240, 12, "#7a5236", 3), rect(980, 360, 26, 50, "#4f7cff", 3), rect(1010, 372, 22, 38, "#ffd23f", 3), circle(1150, 390, 20, "#3fae6a"),
]);

bg("newsroom", "Newsroom", ["#16213e", "#2a3350", "#d6283c"], [
  rect(0, 0, 1280, 560, lin(0, 0, 0, 560, "$0", "$1")),
  // A big screen with a world map feel, centred.
  rect(380, 60, 520, 300, "#0b1020", 10), rect(392, 72, 496, 276, lin(392, 72, 888, 348, "#1f4fa8", "#0e2a63")),
  ...[[460, 160, 60, 40], [560, 140, 90, 70], [690, 170, 70, 50], [780, 220, 50, 60], [520, 250, 40, 50]].map(([x, y, w, h]) => ellipse(x, y, w, h * 0.6, "#3a78d6", { opacity: 0.7 })),
  rect(392, 300, 496, 48, "$2"), rect(410, 316, 220, 16, "#ffffff", 4, { opacity: 0.9 }),
  // Light bars.
  ...[60, 160, 1060, 1160].map((x) => rect(x, 40, 40, 480, lin(0, 40, 0, 520, "#4f7cff", "#16213e"), 8, { opacity: 0.6 })),
  rect(0, 560, 1280, 160, lin(0, 560, 0, 720, "#0e1428", "#05070f")),
  ellipse(640, 600, 420, 36, "#4f7cff", { opacity: 0.12 }),
]);

// ---------------------------------------------------------------- props
const props = [];
const prop = (id, name, category, colors, shapes) => props.push({ id, name, category, colors, shapes });

prop("mic-stand", "Podcast mic", "Studio", ["#2a2f3c", "#9aa3b5"], [
  rect(-6, -150, 12, 150, "$0"), ellipse(0, -4, 46, 8, "$0"),
  rect(-24, -230, 48, 80, "$1", 24), ...[-200, -185, -170].map((y) => line(-18, y, 18, y, "$1d", 2)),
  rect(-30, -160, 60, 10, "$0", 5),
]);
prop("desk-mic", "Desk microphone", "Studio", ["#2a2f3c", "#c9ced8"], [
  ellipse(0, -6, 34, 8, "$0"), line(0, -6, 0, -60, "$0", 6), line(0, -60, 18, -86, "$0", 6),
  rect(4, -130, 36, 56, "$1", 18), ...[-116, -104, -92].map((y) => line(10, y, 34, y, "$1d", 2)),
]);
prop("ring-light", "Ring light", "Studio", ["#2a2f3c", "#fff3c4"], [
  line(0, 0, 0, -260, "$0", 6), poly([[-50, 0], [50, 0], [0, -40]], "#00000000", { stroke: "$0", sw: 5 }),
  circle(0, -320, 70, "#00000000", { stroke: "$1", sw: 18 }), circle(0, -320, 70, "#00000000", { stroke: "#ffffff", sw: 6, opacity: 0.8 }),
]);
prop("camera", "Video camera", "Studio", ["#2a2f3c", "#4f7cff"], [
  line(0, 0, -40, -120, "$0", 5), line(0, 0, 40, -120, "$0", 5), line(0, 0, 0, -120, "$0", 5),
  rect(-60, -190, 110, 70, "$0", 10), circle(70, -155, 26, "$0"), circle(70, -155, 16, "$1"), circle(-40, -175, 6, "#e84a5f"),
]);
prop("podium", "Podium", "Studio", ["#7c5cff", "#ffffff"], [
  poly([[-70, 0], [70, 0], [60, -170], [-60, -170]], "$0"), rect(-80, -190, 160, 22, "$0d", 6), circle(0, -95, 26, "$1", { opacity: 0.85 }),
]);
prop("phone", "Phone", "Things", ["#14161f", "#4f7cff"], [
  rect(-24, -96, 48, 96, "$0", 10), rect(-19, -88, 38, 78, lin(0, -88, 0, -10, "$1l", "$1"), 6), circle(0, -6, 3, "#9aa3b5"),
]);
prop("guitar", "Guitar", "Things", ["#e8a35c", "#7a5236"], [
  ellipse(0, -60, 54, 60, "$0"), ellipse(0, -130, 40, 44, "$0"), circle(0, -95, 15, "#2a2f3c"),
  rect(-7, -290, 14, 170, "$1"), rect(-12, -320, 24, 34, "$1d", 4), rect(-22, -40, 44, 10, "$1d", 3),
]);
prop("book-stack", "Books", "Things", ["#4f7cff", "#e84a5f", "#3fae6a"], [
  rect(-60, -24, 120, 24, "$0", 4), rect(-52, -48, 110, 24, "$1", 4), rect(-58, -72, 104, 24, "$2", 4),
  ...[-12, -36, -60].map((y) => rect(-50, y - 4, 96, 4, "#ffffff", 2, { opacity: 0.6 })),
]);
prop("backpack", "Backpack", "Things", ["#e84a5f", "#2a2f3c"], [
  rect(-50, -150, 100, 150, "$0", 30), rect(-38, -80, 76, 60, "$0d", 14), rect(-30, -110, 60, 8, "$1", 4), path("M -30 -150 Q 0 -190 30 -150", "#00000000", { stroke: "$1", sw: 8 }),
]);
prop("pizza", "Pizza", "Food", ["#f2b84b", "#e84a5f"], [
  ellipse(0, -14, 90, 22, "#c9ced8"), ellipse(0, -20, 80, 18, "$0"), ellipse(0, -21, 70, 14, "$1"),
  ...[[-35, -22], [10, -26], [40, -18], [-10, -16]].map(([x, y]) => circle(x, y, 7, "#b23a48")),
]);
prop("cake", "Birthday cake", "Food", ["#ff9fc4", "#ffffff"], [
  rect(-70, -90, 140, 90, "$0", 10), path("M -70 -70 Q -55 -50 -40 -70 Q -25 -50 -10 -70 Q 5 -50 20 -70 Q 35 -50 50 -70 Q 62 -52 70 -70 L 70 -90 L -70 -90 Z", "$1"),
  ...[-30, 0, 30].flatMap((x) => [rect(x - 4, -130, 8, 40, "#4f7cff", 3), ellipse(x, -138, 6, 10, "#ffd23f")]),
]);
prop("popcorn", "Popcorn", "Food", ["#e84a5f", "#fff6d6"], [
  poly([[-40, 0], [40, 0], [52, -110], [-52, -110]], "#ffffff"), ...[-34, -12, 10, 32].map((x) => poly([[x - 6, 0], [x + 6, 0], [x + 12, -110], [x, -110]], "$0")),
  ...[[-40, -118], [-15, -128], [12, -124], [38, -116], [-28, -136], [4, -142], [28, -134]].map(([x, y]) => circle(x, y, 16, "$1")),
]);
prop("gift", "Gift box", "Party", ["#7c5cff", "#ffd23f"], [
  rect(-50, -90, 100, 90, "$0", 6), rect(-56, -106, 112, 22, "$0l", 5), rect(-8, -106, 16, 106, "$1"),
  path("M 0 -106 Q -40 -150 -30 -110 Z", "$1"), path("M 0 -106 Q 40 -150 30 -110 Z", "$1"),
]);
prop("umbrella", "Umbrella", "Things", ["#4f7cff", "#2a2f3c"], [
  path("M -110 -200 Q 0 -320 110 -200 Q 82 -214 55 -200 Q 27 -214 0 -200 Q -27 -214 -55 -200 Q -82 -214 -110 -200 Z", "$0"),
  line(0, -268, 0, -20, "$1", 5), path("M 0 -20 Q 0 6 -20 0", "#00000000", { stroke: "$1", sw: 5 }),
]);
prop("cat", "Cat", "Animals", ["#f2a65a", "#2a2f3c"], [
  ellipse(0, -40, 50, 38, "$0"), circle(42, -86, 30, "$0"), poly([[24, -108], [32, -136], [44, -112]], "$0"), poly([[46, -112], [60, -134], [66, -104]], "$0"),
  circle(34, -88, 4, "$1"), circle(54, -88, 4, "$1"), path("M -48 -40 Q -90 -60 -76 -110", "#00000000", { stroke: "$0", sw: 12 }),
  ...[-26, 20].map((x) => rect(x - 8, -12, 16, 14, "$0d", 6)),
]);
prop("dog", "Dog", "Animals", ["#c9a27a", "#6b4630"], [
  ellipse(0, -60, 70, 40, "$0"), circle(70, -110, 36, "$0"), ellipse(88, -98, 22, 16, "$0l"), circle(104, -102, 6, "$1"), circle(70, -118, 5, "$1"),
  ellipse(50, -124, 14, 30, "$1", { opacity: 0.9 }), ...[-46, -18, 30, 54].map((x) => rect(x - 9, -30, 18, 30, "$0d", 6)),
  path("M -66 -70 Q -100 -100 -92 -126", "#00000000", { stroke: "$0", sw: 10 }),
]);
prop("fridge", "Fridge", "Home", ["#e7ecf3", "#9aa3b5"], [
  rect(-70, -300, 140, 300, "$0", 14), line(-70, -190, 70, -190, "$1", 3), rect(44, -270, 8, 60, "$1", 4), rect(44, -170, 8, 80, "$1", 4),
  rect(-50, -260, 30, 24, "#ffd23f", 3, { opacity: 0.9 }), circle(-20, -230, 8, "#e84a5f"),
]);
prop("bicycle", "Bicycle", "Outdoors", ["#e84a5f", "#2a2f3c"], [
  circle(-70, -46, 44, "#00000000", { stroke: "$1", sw: 7 }), circle(70, -46, 44, "#00000000", { stroke: "$1", sw: 7 }),
  poly([[-70, -46], [-10, -46], [40, -110], [-30, -110]], "#00000000", { stroke: "$0", sw: 7 }), line(-10, -46, -30, -120, "$0", 7), line(40, -110, 70, -46, "$0", 7),
  rect(-46, -128, 40, 10, "$1", 5), line(40, -110, 34, -140, "$1", 6), line(22, -140, 50, -140, "$1", 6),
]);
prop("rocket", "Rocket", "Fun", ["#ffffff", "#e84a5f"], [
  path("M 0 -260 Q 50 -200 46 -80 L -46 -80 Q -50 -200 0 -260 Z", "$0"), circle(0, -170, 20, "#4f7cff"), circle(0, -170, 14, "#8fd0ff"),
  poly([[-46, -120], [-80, -60], [-46, -80]], "$1"), poly([[46, -120], [80, -60], [46, -80]], "$1"), path("M -26 -80 Q 0 -10 26 -80 Z", "#ffd23f"), path("M -14 -80 Q 0 -40 14 -80 Z", "#ff9f1c"),
]);
prop("trophy-star", "Star", "Fun", ["#ffd23f", "#ff9f1c"], [
  poly([[0, -130], [18, -84], [68, -82], [28, -52], [42, -4], [0, -32], [-42, -4], [-28, -52], [-68, -82], [-18, -84]], "$0", { stroke: "$1", sw: 4 }),
]);
prop("heart", "Heart", "Fun", ["#ff4d6d"], [path("M 0 -10 C -90 -70 -60 -150 0 -110 C 60 -150 90 -70 0 -10 Z", "$0"), ellipse(-30, -104, 12, 8, "#ffffff", { opacity: 0.5 })]);
prop("question", "Question mark", "Fun", ["#4f7cff"], [path("M -34 -150 Q -34 -200 0 -200 Q 40 -200 40 -160 Q 40 -130 10 -116 Q 0 -110 0 -84 L -18 -84 Q -18 -120 12 -136 Q 22 -142 22 -160 Q 22 -184 0 -184 Q -16 -184 -16 -150 Z", "$0"), circle(-9, -54, 14, "$0")]);
prop("exclaim", "Exclamation", "Fun", ["#e84a5f"], [rect(-14, -200, 28, 120, "$0", 12), circle(0, -50, 16, "$0")]);
prop("speech-dots", "Typing dots", "Fun", ["#ffffff", "#9aa3b5"], [rect(-60, -60, 120, 60, "$0", 30), ...[-28, 0, 28].map((x) => circle(x, -30, 10, "$1"))]);

const pack = { id: "scenes", name: "Scenes pack", version: "0.1.0", license: "CC0-1.0", props, backgrounds };
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(pack));
console.log(`wrote ${out}: ${backgrounds.length} backgrounds, ${props.length} props`);
