#!/usr/bin/env python3
"""Generates packs/core/pack.json, the built-in asset pack.

All art here is original and made of simple vector primitives, released under
CC0 so anyone can reuse or remix it. Run: python3 tools/build_core_pack.py
Coordinates: props anchor at their base centre (0,0) with y growing down, so
everything is drawn at negative y. Backgrounds use a 1280x720 stage.
"""
import json, os, random

def rect(x, y, w, h, fill=None, r=0, stroke=None, sw=1, opacity=1):
    d = {"t": "rect", "x": x, "y": y, "w": w, "h": h, "r": r}
    return style(d, fill, stroke, sw, opacity)

def circle(cx, cy, r, fill=None, stroke=None, sw=1, opacity=1):
    return style({"t": "circle", "cx": cx, "cy": cy, "r": r}, fill, stroke, sw, opacity)

def ellipse(cx, cy, rx, ry, fill=None, stroke=None, sw=1, opacity=1):
    return style({"t": "ellipse", "cx": cx, "cy": cy, "rx": rx, "ry": ry}, fill, stroke, sw, opacity)

def poly(points, fill=None, stroke=None, sw=1, opacity=1):
    return style({"t": "poly", "points": points}, fill, stroke, sw, opacity)

def line(x1, y1, x2, y2, stroke, sw=2, opacity=1):
    return style({"t": "line", "x1": x1, "y1": y1, "x2": x2, "y2": y2}, None, stroke, sw, opacity)

def path(d, fill=None, stroke=None, sw=1, opacity=1):
    return style({"t": "path", "d": d}, fill, stroke, sw, opacity)

def lin(x0, y0, x1, y1, *stops):
    return {"lin": [x0, y0, x1, y1], "stops": list(stops)}

def style(d, fill, stroke, sw, opacity):
    if fill is not None: d["fill"] = fill
    if stroke is not None: d["stroke"] = stroke; d["sw"] = sw
    if opacity != 1: d["opacity"] = opacity
    return d

# ---------------------------------------------------------------- props
props = []
def prop(id, name, category, colors, shapes):
    props.append({"id": id, "name": name, "category": category, "colors": colors, "shapes": shapes})

prop("desk", "Desk", "Office", ["#b9875a", "#8a6140"], [
    rect(-150, -150, 300, 22, "$0", 6),
    rect(-150, -132, 300, 8, "$0d"),
    rect(-138, -128, 16, 128, "$1"),
    rect(122, -128, 16, 128, "$1"),
    rect(30, -126, 92, 70, "$0d", 4),
    rect(66, -98, 20, 6, "$1d", 3),
])
prop("chair", "Office chair", "Office", ["#3d4b6b", "#2a2f3c"], [
    rect(-44, -190, 88, 96, "$0", 22),
    rect(-50, -104, 100, 22, "$0d", 10),
    rect(-6, -84, 12, 54, "$1"),
    poly([[-46, -10], [46, -10], [30, -30], [-30, -30]], "$1"),
    circle(-40, -6, 7, "$1d"), circle(40, -6, 7, "$1d"), circle(0, -6, 7, "$1d"),
])
prop("laptop", "Laptop", "Office", ["#c9ced8", "#4f7cff"], [
    poly([[-54, -6], [54, -6], [64, 0], [-64, 0]], "$0d"),
    rect(-50, -76, 100, 70, "$0", 6),
    rect(-43, -69, 86, 56, "$1", 3),
    rect(-35, -60, 40, 6, "#ffffff", 3, opacity=0.7),
    rect(-35, -48, 60, 6, "#ffffff", 3, opacity=0.5),
    circle(0, -40, 0.1, "#ffffff"),
])
prop("plant", "Potted plant", "Office", ["#3fae6a", "#d9784c"], [
    path("M0 -70 C -50 -90 -70 -150 -40 -190 C -20 -150 -10 -110 0 -70 Z", "$0d"),
    path("M0 -70 C 50 -100 70 -160 40 -200 C 20 -150 10 -110 0 -70 Z", "$0"),
    path("M0 -70 C -10 -130 0 -190 10 -230 C 26 -180 18 -120 0 -70 Z", "$0l"),
    path("M0 -70 C -60 -60 -90 -100 -96 -130 C -60 -120 -30 -100 0 -70 Z", "$0"),
    path("M0 -70 C 60 -60 90 -100 96 -124 C 60 -118 30 -100 0 -70 Z", "$0d"),
    poly([[-40, -76], [40, -76], [30, 0], [-30, 0]], "$1"),
    rect(-46, -84, 92, 16, "$1d", 4),
])
prop("lamp", "Floor lamp", "Home", ["#f3d36b", "#2f3442"], [
    ellipse(0, -6, 40, 8, "$1"),
    rect(-4, -300, 8, 296, "$1"),
    poly([[-52, -290], [52, -290], [30, -370], [-30, -370]], "$0"),
    poly([[-52, -290], [52, -290], [46, -280], [-46, -280]], "$0d"),
])
prop("sofa", "Sofa", "Home", ["#d9665b", "#7a4a33"], [
    rect(-170, -150, 340, 90, "$0d", 30),
    rect(-150, -86, 300, 40, "$0", 14),
    rect(-190, -110, 50, 80, "$0", 22),
    rect(140, -110, 50, 80, "$0", 22),
    rect(-176, -46, 352, 30, "$0d", 10),
    rect(-160, -18, 14, 18, "$1"), rect(146, -18, 14, 18, "$1"),
    line(0, -84, 0, -50, "$0dd", 3),
])
prop("bookshelf", "Bookshelf", "Home", ["#9a6a43", "#e85d75", "#4f7cff", "#f2b84b", "#3fae6a"], [
    rect(-90, -320, 180, 320, "$0", 6),
    rect(-78, -306, 156, 292, "$0dd", 2),
] + [rect(-78, -306 + 98 * i + 90, 156, 10, "$0") for i in range(3)] + [
    rect(-74 + 18 * j, -306 + 98 * i + 90 - (60 + (i * 7 + j * 13) % 22), 15, 60 + (i * 7 + j * 13) % 22,
         "$" + str(1 + (i + j) % 4), 2)
    for i in range(3) for j in range(8) if (i * 3 + j) % 5 != 4
])
prop("tree", "Tree", "Outdoors", ["#3c9a5f", "#7a5236"], [
    path("M-14 0 L -10 -180 L 10 -180 L 14 0 Z", "$1"),
    circle(-60, -230, 70, "$0d"),
    circle(60, -230, 70, "$0d"),
    circle(0, -290, 86, "$0"),
    circle(-30, -200, 60, "$0"),
    circle(40, -196, 56, "$0"),
    circle(-26, -310, 34, "$0l", opacity=0.6),
])
prop("bush", "Bush", "Outdoors", ["#4cae6b"], [
    circle(-50, -40, 44, "$0d"), circle(50, -40, 44, "$0d"), circle(0, -60, 56, "$0"),
    rect(-90, -40, 180, 40, "$0d", 10),
])
prop("bench", "Park bench", "Outdoors", ["#b07a4b", "#3b3f4c"], [
    rect(-130, -150, 260, 18, "$0", 5), rect(-130, -124, 260, 18, "$0", 5),
    rect(-140, -78, 280, 18, "$0d", 5),
    rect(-120, -150, 10, 150, "$1"), rect(110, -150, 10, 150, "$1"),
    rect(-120, -64, 10, 64, "$1"),
])
prop("car", "Car", "Outdoors", ["#e8574d", "#2b2f3a", "#bfe3ff"], [
    path("M-170 -40 L -170 -90 C -170 -110 -150 -116 -130 -118 L -90 -170 C -80 -182 -70 -186 -50 -186 L 70 -186 C 90 -186 100 -180 112 -166 L 140 -120 C 166 -116 180 -106 180 -86 L 180 -40 Z", "$0"),
    path("M-78 -122 L -50 -166 L -6 -166 L -6 -122 Z", "$2"),
    path("M6 -122 L 6 -166 L 68 -166 C 80 -166 88 -160 94 -152 L 114 -122 Z", "$2"),
    rect(-176, -56, 356, 14, "$0d", 6),
    circle(-100, -36, 34, "$1"), circle(-100, -36, 14, "#c9ced8"),
    circle(110, -36, 34, "$1"), circle(110, -36, 14, "#c9ced8"),
    rect(160, -100, 18, 12, "#ffe48a", 4),
])
prop("coffee", "Coffee cup", "Office", ["#ffffff", "#6b4630"], [
    path("M-22 -60 L 22 -60 L 18 0 L -18 0 Z", "$0"),
    rect(-24, -66, 48, 10, "$0d", 4),
    rect(-22, -46, 44, 18, "$1"),
    path("M18 -50 C 36 -50 36 -20 16 -20", None, "$0d", 6),
])
prop("sign", "Sign board", "Outdoors", ["#ffffff", "#7a5236", "#2f3442"], [
    rect(-8, -170, 16, 170, "$1"),
    rect(-110, -260, 220, 120, "$0", 12, "$2", 6),
    rect(-80, -226, 160, 12, "$2", 6, opacity=0.25),
    rect(-80, -200, 120, 12, "$2", 6, opacity=0.25),
])
prop("box", "Cardboard box", "Home", ["#d2a06a"], [
    rect(-70, -110, 140, 110, "$0", 4),
    rect(-70, -110, 140, 26, "$0d", 4),
    rect(-12, -110, 24, 50, "#f0e2c4", 0, opacity=0.8),
])
prop("door", "Door", "Home", ["#8a5a3c", "#e3c16f"], [
    rect(-80, -330, 160, 330, "$0d", 4),
    rect(-70, -320, 140, 320, "$0", 3),
    rect(-54, -300, 108, 120, "$0d", 6, opacity=0.5),
    rect(-54, -160, 108, 140, "$0d", 6, opacity=0.5),
    circle(48, -160, 8, "$1"),
])
prop("tv", "Television", "Home", ["#2a2f3c", "#4f7cff"], [
    rect(-150, -250, 300, 180, "$0", 12),
    rect(-138, -238, 276, 156, lin(0, -238, 0, -82, "$1l", "$1d"), 4),
    rect(-10, -70, 20, 40, "$0"),
    rect(-70, -34, 140, 14, "$0", 7),
    rect(-120, -20, 240, 20, "$0l", 4, opacity=0.4),
])
prop("whiteboard", "Whiteboard", "Office", ["#ffffff", "#9aa3b5", "#4f7cff", "#e85d75"], [
    rect(-8, -120, 16, 120, "$1"), rect(-90, -10, 180, 10, "$1", 5),
    rect(-160, -330, 320, 210, "$0", 6, "$1", 8),
    path("M-120 -200 L -60 -260 L 0 -220 L 60 -290 L 110 -250", None, "$2", 6),
    rect(-120, -170, 100, 10, "$3", 5, opacity=0.8),
    rect(-120, -150, 150, 10, "$1", 5, opacity=0.6),
])
prop("balloon", "Balloons", "Party", ["#e85d75", "#4f7cff", "#f2b84b"], [
    path("M0 0 C -10 -60 -40 -110 -40 -160", None, "#7b8090", 2),
    path("M0 0 C 0 -80 10 -140 10 -200", None, "#7b8090", 2),
    path("M0 0 C 10 -60 40 -100 46 -150", None, "#7b8090", 2),
    ellipse(-40, -200, 34, 42, "$0"), ellipse(10, -244, 34, 42, "$1"), ellipse(48, -190, 34, 42, "$2"),
    ellipse(-50, -214, 8, 12, "#ffffff", opacity=0.5), ellipse(0, -258, 8, 12, "#ffffff", opacity=0.5),
    ellipse(38, -204, 8, 12, "#ffffff", opacity=0.5),
])
prop("trophy", "Trophy", "Party", ["#f2b84b", "#7a5236"], [
    rect(-40, -24, 80, 24, "$1", 4),
    rect(-10, -60, 20, 40, "$0d"),
    path("M-46 -150 L 46 -150 C 46 -90 20 -60 0 -60 C -20 -60 -46 -90 -46 -150 Z", "$0"),
    path("M-46 -140 C -76 -140 -76 -96 -40 -96", None, "$0", 8),
    path("M46 -140 C 76 -140 76 -96 40 -96", None, "$0", 8),
    rect(-30, -140, 12, 50, "#ffffff", 6, opacity=0.35),
])
prop("cloud", "Cloud", "Sky", ["#ffffff"], [
    circle(-50, -40, 36, "$0"), circle(0, -60, 50, "$0"), circle(52, -40, 36, "$0"),
    rect(-86, -40, 172, 40, "$0", 20),
])

# ---------------------------------------------------------------- backgrounds
backgrounds = []
def bg(id, name, colors, shapes):
    backgrounds.append({"id": id, "name": name, "colors": colors, "shapes": shapes})

W, H = 1280, 720
rnd = random.Random(7)

def window(x, y, w, h, sky_top="#8fd0ff", sky_bot="#d8f1ff", frame="#ffffff"):
    return [
        rect(x - 10, y - 10, w + 20, h + 20, frame, 6),
        rect(x, y, w, h, lin(0, y, 0, y + h, sky_top, sky_bot)),
        circle(x + w * 0.3, y + h * 0.4, 26, "#ffffff", opacity=0.9),
        circle(x + w * 0.3 + 30, y + h * 0.4 + 6, 20, "#ffffff", opacity=0.9),
        rect(x + w / 2 - 5, y, 10, h, frame),
        rect(x, y + h / 2 - 5, w, 10, frame),
        rect(x - 18, y + h + 6, w + 36, 14, frame, 4),
    ]

bg("office", "Office", ["#e7ecf5", "#c99a6b"], [
    rect(0, 0, W, 560, lin(0, 0, 0, 560, "$0l", "$0")),
    rect(0, 540, W, 20, "$0d"),
    rect(0, 560, W, 160, lin(0, 560, 0, 720, "$1", "$1d")),
] + [line(x, 560, x - 60, 720, "$1dd", 2, 0.25) for x in range(80, 1400, 140)] + window(110, 120, 320, 240) + [
    rect(820, 130, 220, 150, "#ffffff", 6), rect(836, 146, 188, 118, lin(0, 146, 0, 264, "#9bd1a5", "#4f9a7a"), 2),
    poly([[836, 264], [910, 190], [970, 240], [1000, 210], [1024, 240], [1024, 264]], "#3d7d63"),
    rect(560, 0, 160, 18, "#d0d6e2"), rect(600, 18, 80, 8, "#fff7d6", 4),
])

bg("park", "Park", ["#8fd0ff", "#7cc86b"], [
    rect(0, 0, W, 520, lin(0, 0, 0, 520, "$0", "$0ll")),
    circle(1080, 120, 60, "#ffe48a"), circle(1080, 120, 80, "#ffe48a", opacity=0.3),
    path("M0 470 C 200 360 380 420 520 440 C 700 360 900 380 1280 450 L 1280 560 L 0 560 Z", "$1d", opacity=0.6),
    rect(0, 500, W, 220, lin(0, 500, 0, 720, "$1", "$1d")),
    path("M520 720 C 600 640 700 590 860 560 L 940 560 C 820 600 760 660 760 720 Z", "#e8d3a6"),
] + [circle(x, y, r, "#ffffff", opacity=0.95) for (x, y, r) in [(200, 110, 34), (240, 100, 44), (285, 115, 32), (640, 80, 28), (676, 70, 38), (712, 84, 28)]])

city = [rect(0, 0, W, 720, lin(0, 0, 0, 560, "$0", "$1"))]
x = -20
while x < W:
    w = rnd.randint(110, 180); h = rnd.randint(220, 420)
    shade = rnd.choice(["#3b4263", "#454d72", "#343a58", "#4a4f78"])
    city.append(rect(x, 560 - h, w, h, shade))
    for wy in range(560 - h + 20, 540, 34):
        for wx in range(x + 14, x + w - 20, 28):
            if rnd.random() < 0.55:
                city.append(rect(wx, wy, 14, 18, "#ffd97a", 2, opacity=0.85))
    x += w + rnd.randint(4, 20)
city += [rect(0, 560, W, 160, "#4a4f5e"), rect(0, 560, W, 14, "#8a90a0")]
city += [rect(x, 640, 70, 8, "#f2f2f2", 2, opacity=0.8) for x in range(20, W, 140)]
bg("city", "City street (dusk)", ["#ff9e7a", "#6a5b9a"], city)

bg("living-room", "Living room", ["#f1dcc3", "#a26e4a", "#c45d4f"], [
    rect(0, 0, W, 560, "$0"),
    rect(0, 0, W, 300, "$0l", opacity=0.5),
    rect(0, 540, W, 20, "#ffffff"),
    rect(0, 560, W, 160, lin(0, 560, 0, 720, "$1", "$1d")),
    ellipse(640, 640, 380, 46, "$2", opacity=0.85),
    ellipse(640, 640, 330, 34, "$2l", opacity=0.5),
] + window(860, 110, 280, 230) + [
    rect(120, 150, 260, 16, "#7a5236", 4),
    rect(140, 96, 40, 54, "#4f7cff", 3), rect(186, 110, 30, 40, "#3fae6a", 3), rect(222, 90, 36, 60, "#f2b84b", 3),
    circle(320, 124, 26, "#ffffff"), circle(320, 124, 22, "#3d4b6b"), line(320, 124, 320, 108, "#ffffff", 3), line(320, 124, 332, 124, "#ffffff", 3),
])

bg("classroom", "Classroom", ["#f4ead2", "#2f5d50", "#b98a5c"], [
    rect(0, 0, W, 560, "$0"),
    rect(0, 380, W, 180, "$0d", opacity=0.6),
    rect(0, 560, W, 160, lin(0, 560, 0, 720, "$2", "$2d")),
    rect(250, 90, 780, 300, "#8a5a3c", 8),
    rect(266, 106, 748, 268, "$1"),
    path("M320 180 L 420 180 M 320 230 L 520 230 M 320 280 L 460 280", None, "#ffffff", 5, opacity=0.6),
    path("M700 300 L 760 200 L 820 260 L 900 160", None, "#ffe48a", 5, opacity=0.8),
    rect(266, 374, 748, 12, "#8a5a3c"),
    rect(560, 30, 160, 40, "#ffffff", 8), circle(640, 50, 12, "#e85d75"),
])

bg("beach", "Beach", ["#7fd3ff", "#2fa7c9", "#f5dc9f"], [
    rect(0, 0, W, 420, lin(0, 0, 0, 420, "$0", "$0ll")),
    circle(300, 140, 70, "#fff1a6"),
    rect(0, 400, W, 140, lin(0, 400, 0, 540, "$1", "$1d")),
] + [path(f"M{x} {y} q 20 -10 40 0", None, "#ffffff", 3, 0.6) for x, y in [(120, 440), (420, 470), (760, 430), (1040, 480), (900, 510), (240, 500)]] + [
    path("M0 520 C 300 500 700 540 1280 510 L 1280 720 L 0 720 Z", "$2"),
    path("M0 540 C 300 520 700 560 1280 530", None, "#ffffff", 8, 0.7),
])

night = [rect(0, 0, W, 720, lin(0, 0, 0, 560, "$0", "$1"))]
night += [circle(rnd.randint(0, W), rnd.randint(0, 420), rnd.choice([1.5, 2, 2.5]), "#ffffff", opacity=rnd.choice([0.5, 0.8, 1])) for _ in range(90)]
night += [circle(1030, 140, 54, "#fdf6d8"), circle(1052, 128, 50, "$0")]
night += [path("M0 520 C 240 430 420 470 640 500 C 860 440 1060 450 1280 510 L 1280 720 L 0 720 Z", "#1d2a3c"),
          rect(0, 590, W, 130, "#152030")]
bg("night", "Night sky", ["#0f1733", "#3a3f78"], night)

bg("studio", "Studio (plain)", ["#eef1f8", "#c9d2e6"], [
    rect(0, 0, W, 720, lin(0, 0, 0, 720, "$0", "$1")),
    ellipse(640, 640, 420, 40, "#000000", opacity=0.06),
])

# ---------------------------------------------------------------- actions
def key(t, **pose):
    return {"t": t, "pose": pose}

REST = dict(ls=8, le=6, rs=8, re=6)
def rest(**kw):
    d = dict(REST); d.update(kw); return d

actions = [
    {"id": "idle", "name": "Idle", "duration": 3.0, "keys": [
        key(0, **rest()), key(0.5, **rest(bob=2.5, ls=10, rs=10, le=9, re=9, head=1.5))]},
    {"id": "talk", "name": "Talk", "duration": 1.8, "keys": [
        key(0, **rest(rs=22, re=38)),
        key(0.25, **rest(rs=34, re=72, head=3, bob=1)),
        key(0.5, **rest(rs=16, re=28, ls=26, le=52, head=-2)),
        key(0.75, **rest(ls=14, le=18, rs=30, re=58, head=1, bob=1))]},
    {"id": "wave", "name": "Wave", "duration": 1.0, "keys": [
        key(0, **rest(rs=140, re=20, head=4)), key(0.25, **rest(rs=145, re=-25, head=4, bob=1)),
        key(0.5, **rest(rs=140, re=20, head=4)), key(0.75, **rest(rs=145, re=-25, head=4, bob=1))]},
    {"id": "walk", "name": "Walk", "duration": 0.9, "keys": [
        key(0, bob=0, ls=20, le=12, rs=2, re=6, lh=8, rh=-4),
        key(0.25, bob=5, ls=10, le=8, rs=10, re=8, lh=-4, lk=30, rh=2),
        key(0.5, bob=0, ls=2, le=6, rs=20, re=12, lh=-4, rh=8),
        key(0.75, bob=5, ls=10, le=8, rs=10, re=8, rh=-4, rk=30, lh=2)]},
    {"id": "run", "name": "Run", "duration": 0.55, "keys": [
        key(0, bob=0, lean=4, ls=40, le=60, rs=-10, re=70, lh=14, rh=-8, rise=6),
        key(0.25, bob=7, lean=4, ls=15, le=60, rs=15, re=60, lh=-6, lk=50, rh=4),
        key(0.5, bob=0, lean=4, ls=-10, le=70, rs=40, re=60, lh=-8, rh=14, rise=6),
        key(0.75, bob=7, lean=4, ls=15, le=60, rs=15, re=60, rh=-6, rk=50, lh=4)]},
    {"id": "jump", "name": "Jump", "duration": 1.2, "keys": [
        key(0, **rest()),
        key(0.18, bob=14, ls=-15, le=30, rs=-15, re=30, lh=12, lk=-24, rh=12, rk=-24),
        key(0.42, rise=110, ls=150, le=10, rs=150, re=10, lh=10, rh=10, lk=-10, rk=-10, head=-3),
        key(0.66, bob=10, ls=30, le=20, rs=30, re=20, lh=10, lk=-20, rh=10, rk=-20),
        key(0.85, **rest())]},
    {"id": "cheer", "name": "Cheer", "duration": 0.8, "keys": [
        key(0, ls=155, le=12, rs=155, re=12, head=-3),
        key(0.5, ls=165, le=-12, rs=165, re=-12, rise=14, head=3)]},
    {"id": "think", "name": "Think", "duration": 3.0, "keys": [
        key(0, ls=-38, le=-58, rs=-88, re=-116, head=6, look=0.6),
        key(0.5, ls=-36, le=-60, rs=-86, re=-118, head=8, bob=1, look=0.8)]},
    {"id": "point", "name": "Point", "duration": 2.0, "keys": [
        key(0, ls=8, le=6, rs=92, re=-4, head=5, look=1),
        key(0.5, ls=9, le=8, rs=96, re=-2, head=6, look=1, bob=1)]},
    {"id": "shrug", "name": "Shrug", "duration": 1.6, "keys": [
        key(0, ls=36, le=88, rs=36, re=88, bob=-3, head=-6),
        key(0.5, ls=40, le=96, rs=40, re=96, bob=-5, head=6)]},
    {"id": "clap", "name": "Clap", "duration": 0.5, "keys": [
        key(0, ls=-18, le=-62, rs=-18, re=-62, head=2),
        key(0.5, ls=10, le=-62, rs=10, re=-62, head=-1)]},
    {"id": "dance", "name": "Dance", "duration": 1.0, "keys": [
        key(0, lean=-8, ls=120, le=30, rs=30, re=60, lh=12, rh=-4, head=-6),
        key(0.25, bob=8, lean=0, ls=60, le=60, rs=60, re=60, lk=20, rk=20),
        key(0.5, lean=8, ls=30, le=60, rs=120, re=30, rh=12, lh=-4, head=6),
        key(0.75, bob=8, lean=0, ls=60, le=60, rs=60, re=60, lk=20, rk=20)]},
    {"id": "sad", "name": "Sad", "duration": 3.0, "keys": [
        key(0, ls=2, le=2, rs=2, re=2, head=10, bob=6, lean=2),
        key(0.5, ls=3, le=3, rs=3, re=3, head=12, bob=8, lean=2)]},
]

pack = {
    "id": "core",
    "name": "Core pack",
    "version": "0.1.0",
    "license": "CC0-1.0",
    "props": props,
    "backgrounds": backgrounds,
    "actions": actions,
}
out = os.path.join(os.path.dirname(__file__), "..", "packs", "core", "pack.json")
with open(out, "w") as f:
    json.dump(pack, f, separators=(",", ":"))
print(f"wrote {out}: {len(props)} props, {len(backgrounds)} backgrounds, {len(actions)} actions")
