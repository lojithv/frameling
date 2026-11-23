"""Generate packs/poses/pack.json: the built-in pose library.

Poses are joint values on the shared skeleton (see crates/core/src/rig.rs):
arm and leg angles are degrees from hanging straight down, positive raising
the limb outward on its own side; elbows and knees add to that. `drop` lowers
the hips (legs bend), `fold` shortens the thighs (sitting, seen from the
front), `tilt` rotates the whole body (lying) and `ground` rests the lowest
point of the body on the floor. Leg angles for crouching and sitting come from
the same two-bone IK the walk cycle uses. Run: python3 tools/build_pose_pack.py
"""
import json, math, os

THIGH, SHIN = 46.0, 44.0
REACH = THIGH + SHIN


def leg_ik(u, v, l1=THIGH, l2=SHIN, bend=1.0):
    d = max(1.0, min(math.hypot(u, v), l1 + l2 - 0.01))
    theta = math.atan2(u, v)
    alpha = math.acos(max(-1, min(1, (l1 * l1 + d * d - l2 * l2) / (2 * l1 * d))))
    a1 = theta + bend * alpha
    ku, kv = math.sin(a1) * l1, math.cos(a1) * l1
    s = math.atan2(u - ku, v - kv)
    return round(math.degrees(a1), 1), round(math.degrees(s - a1), 1)


def legs(drop, out=4.0, bend=1.0, thigh=THIGH):
    """Both feet flat on the floor, `out` px outside the hips, hips dropped by `drop`."""
    h, k = leg_ik(out, REACH - drop, thigh, SHIN, bend)
    return {"drop": drop, "lh": h, "lk": k, "rh": h, "rk": k}


UPPER_ARM, FOREARM = 42.0, 40.0
SHOULDER = (35.0, -198.0)  # regular build, screen-right shoulder; mirrored for the left


def arm_to(x, y, side=1.0, elbow_out=True):
    """Shoulder and elbow angles that put the hand at (x, y) (character space, feet at 0)."""
    u = side * (x - side * SHOULDER[0])
    v = y - SHOULDER[1]
    s, e = leg_ik(u, v, UPPER_ARM, FOREARM, -1.0 if elbow_out else 1.0)
    return s, e


def right_hand(x, y, **kw):
    s, e = arm_to(x, y, 1.0, **kw)
    return {"rs": s, "re": e}


ARMS_DOWN = {"ls": 6, "le": 6, "rs": 6, "re": 6}
CROSSED = {"ls": -32, "le": -104, "rs": -40, "re": -96}
PHONE = {**right_hand(50, -262), "head": 7, "look": -0.3}
CHEER = {"ls": 155, "le": 12, "rs": 155, "re": 12, "head": -3}
LAP = {"ls": -4, "le": -38, "rs": -4, "re": -38}

SIT_THIGH = THIGH * 0.25
sit = {"fold": 0.75, "lh": 8, "lk": -8, "rh": 8, "rk": -8, "drop": round(REACH - (SIT_THIGH * math.cos(math.radians(8)) + SHIN), 1), "ground": 1}
kneel = {"lh": 4, "lk": -4, "rh": 4, "rk": -4, "kneel": 0.88, "ground": 1}

P = []
def pose(id, name, cat, tags, **p):
    P.append({"id": id, "name": name, "category": cat, "tags": tags.split(), "pose": p})

S, G, K, L, M = "Standing", "Gestures", "Sitting and kneeling", "Lying", "Motion keys"
pose("stand", "Stand", S, "neutral idle default straight", **ARMS_DOWN)
pose("relaxed", "Relaxed", S, "casual weight shift easy", ls=4, le=4, rs=12, re=20, head=3, lean=-1.5, lh=4, rk=6)
pose("hands-on-hips", "Hands on hips", S, "confident bossy impatient", ls=48, le=-72, rs=48, re=-72, lh=6, rh=6)
pose("arms-crossed", "Arms crossed", S, "stern waiting annoyed closed", lh=4, rh=4, **CROSSED)
pose("fists", "Fists clenched", S, "angry furious ready fight", ls=16, le=34, rs=16, re=34, bob=2, head=3, lh=9, rh=9)
pose("shrug", "Shrug", S, "dunno unsure whatever", ls=38, le=92, rs=38, re=92, bob=-4, head=-6)
pose("thinking", "Thinking", S, "hand on chin ponder idea", ls=-38, le=-58, **right_hand(18, -214, elbow_out=False), head=6, look=0.6)
pose("facepalm", "Facepalm", S, "embarrassed oops frustrated", ls=4, le=6, **right_hand(40, -256), head=10, bob=3, look=-0.3)
pose("scared", "Scared", S, "afraid fear cower shock", ls=62, le=-112, rs=62, re=-112, head=-4, **legs(8, 10))
pose("tiptoe", "Tiptoe", S, "sneak quiet careful", ls=32, le=44, rs=32, re=44, rise=8, head=-4, look=0.8)
pose("arms-out", "Arms out", S, "welcome open hug ta-da", ls=82, le=4, rs=82, re=4, head=-2)
pose("hands-up", "Hands up", S, "surrender stop don't shoot", ls=118, le=-68, rs=118, re=-68, head=-3)
pose("point", "Point", G, "show that there direct", ls=8, le=6, rs=92, re=-4, head=5, look=1)
pose("wave", "Wave", G, "hello hi bye greet", ls=8, le=6, rs=140, re=20, head=4)
pose("talk", "Talk, one hand", G, "explain conversation speak", ls=8, le=6, rs=34, re=72, head=3, bob=1)
pose("talk-both", "Talk, both hands", G, "explain present discuss", ls=28, le=56, rs=32, re=60, head=-2)
pose("thumbs-up", "Thumbs up", G, "good ok approve like", ls=8, le=6, rs=32, re=118, thumb=1, head=4)
pose("phone-call", "Phone call", G, "phone call ear talk", ls=6, le=6, **PHONE)
pose("cheer", "Cheer", G, "celebrate win yay hooray", **CHEER)
pose("clap", "Clap", G, "applause clapping well done", ls=-18, le=-62, rs=-18, re=-62, head=2)
pose("salute", "Salute", G, "respect military yes sir", ls=6, le=6, **right_hand(28, -277), head=-2)
pose("presenting", "Presenting", G, "show introduce this is", ls=8, le=6, rs=58, re=-18, head=4, look=1)
pose("sit-chair", "Sit on a chair", K, "sit seated chair desk office", **sit, **LAP)
pose("sit-crossed", "Sit, arms crossed", K, "sit seated waiting stern", **sit, **CROSSED)
pose("sit-phone", "Sit on the phone", K, "sit seated phone call", **sit, ls=-4, le=-38, **PHONE)
pose("sit-floor", "Sit on the floor", K, "cross-legged floor meditate relax", lh=78, lk=-152, rh=78, rk=-152, fold=0.2, ls=32, le=28, rs=32, re=28, ground=1)
pose("kneel", "Kneel", K, "kneeling pray floor", **kneel, ls=6, le=10, rs=6, re=10)
pose("kneel-crossed", "Kneel, arms crossed", K, "kneeling waiting", **kneel, **CROSSED)
pose("kneel-cheer", "Kneel and cheer", K, "kneeling celebrate win", **kneel, **CHEER)
pose("crouch", "Crouch", K, "squat hide low duck", ls=-14, le=-44, rs=-14, re=-44, ground=1, head=4, **legs(44, 8))
pose("lie-back", "Lie on the back", L, "sleep rest floor lying down", tilt=-90, ground=1, ls=4, le=4, rs=4, re=4)
pose("lie-side", "Lie on the side", L, "sleep nap curled lying", tilt=-90, ground=1, lh=22, lk=46, rh=18, rk=40, ls=118, le=40, rs=-10, re=-30)
pose("lie-front", "Lie on the front", L, "fly superman crawl lying", tilt=90, ground=1, ls=160, le=0, rs=160, re=0)
pose("jump-crouch", "Jump crouch", M, "jump prepare spring wind-up", ls=-28, le=24, rs=-28, re=24, head=4, **legs(26, 6))
pose("jump-peak", "Jump peak", M, "jump air leap", rise=60, ls=150, le=10, rs=150, re=10, lh=10, lk=-10, rh=10, rk=-10, head=-3)

out = {"id": "poses", "name": "Pose library", "version": "1", "license": "CC0-1.0", "poses": P}
path = os.path.join(os.path.dirname(__file__), "..", "packs", "poses", "pack.json")
json.dump(out, open(path, "w"), indent=1)
print(f"{len(P)} poses (+ walk-stride and run-stride from the walk cycle) -> {os.path.normpath(path)}")
