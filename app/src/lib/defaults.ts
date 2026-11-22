// Factories for new items and scenes, plus the character presets shown in the library.

import type { Item, Look, Scene, TextStyle, CharacterKind, TextKind } from "./types";

export const uid = (prefix: string) =>
  `${prefix}-${(globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2)).slice(0, 8)}`;

const baseLook: Look = {
  skin: "#f1c29a",
  hair: "#3b2a20",
  hairStyle: "short",
  top: "#4f7cff",
  bottom: "#2f3a56",
  shoes: "#2a221e",
  build: "regular",
  accessory: "none",
  accent: "#e84a5f",
};

/** Ready-made characters. Everyone can restyle them in the inspector. */
export const CHARACTER_PRESETS: { name: string; look: Look }[] = [
  { name: "Maya", look: { ...baseLook, skin: "#c68b62", hair: "#231a16", hairStyle: "long", top: "#f26b5b", accessory: "glasses" } },
  { name: "Leo", look: { ...baseLook, skin: "#f3c9a5", hair: "#d99a3e", hairStyle: "spiky", top: "#3fae6a", bottom: "#454d72", build: "broad", accessory: "tie", accent: "#4f7cff" } },
  { name: "Ava", look: { ...baseLook, skin: "#8d5a3b", hair: "#1b1311", hairStyle: "curly", top: "#9b5de5", bottom: "#2b2d42", accessory: "bow", accent: "#ffb703", build: "slim" } },
  { name: "Sam", look: { ...baseLook, skin: "#e9b48a", hair: "#5a3825", hairStyle: "short", top: "#ffb703", bottom: "#3a405a", accessory: "cap", accent: "#e63946" } },
  { name: "Kai", look: { ...baseLook, skin: "#d7a37a", hair: "#121212", hairStyle: "bun", top: "#219ebc", bottom: "#23395b", build: "slim" } },
  { name: "Ray", look: { ...baseLook, skin: "#b07850", hair: "#2d2d2d", hairStyle: "bald", top: "#e76f51", bottom: "#264653", build: "broad", accessory: "glasses" } },
];

export const SKIN_TONES = ["#f9d7bd", "#f1c29a", "#e9b48a", "#d7a37a", "#c68b62", "#b07850", "#8d5a3b", "#5e3b26"];
export const PALETTE = [
  "#1f2233", "#ffffff", "#e84a5f", "#f26b5b", "#ffb703", "#f2d06b", "#3fae6a", "#2a9d8f",
  "#219ebc", "#4f7cff", "#9b5de5", "#f15bb5", "#8a5a3c", "#3b2a20", "#d99a3e", "#9aa3b5",
];

function baseItem(id: string, name: string, x: number, y: number): Omit<Item, "kind"> {
  return {
    id, name, start: 0, end: null, x, y, scale: 1, rotation: 0, flip: false, opacity: 1,
    enter: "none", exit: "none", motion: null, locked: false,
  };
}

export function newCharacter(look: Look, name: string, x = 640, y = 640): Item {
  const kind: CharacterKind = {
    type: "character", look: { ...look }, action: "idle", expression: "neutral", speed: 1, autoWalk: true, autoTalk: true,
    voice: null, voiceSpeed: 1,
  };
  return { ...baseItem(uid("char"), name, x, y), scale: 0.95, kind };
}

export function newProp(prop: string, name: string, x = 640, y = 640): Item {
  return { ...baseItem(uid("prop"), name, x, y), kind: { type: "prop", prop, colors: [] } };
}

export const TEXT_PRESETS: { style: TextStyle; label: string; sample: string }[] = [
  { style: "bubble", label: "Speech bubble", sample: "Hello there!" },
  { style: "thought", label: "Thought bubble", sample: "Hmm..." },
  { style: "caption", label: "Caption", sample: "Meanwhile, at the office..." },
  { style: "title", label: "Title", sample: "Chapter One" },
  { style: "plain", label: "Plain text", sample: "Text" },
];

export function newText(style: TextStyle, text: string, x = 640, y = 200): Item {
  const kind: TextKind = {
    type: "text", text, style, size: 28, color: "#1f2233", fill: "#ffffff", tail: "bottomLeft", width: 320, typewriter: 0,
  };
  if (style === "caption") Object.assign(kind, { size: 30, color: "#ffffff", fill: "#141726cc", width: 900, tail: "none" });
  if (style === "title") Object.assign(kind, { size: 64, color: "#ffffff", width: 1000, tail: "none" });
  if (style === "plain") Object.assign(kind, { tail: "none", width: 600 });
  if (style === "caption") y = 670;
  return { ...baseItem(uid("text"), labelFor(style), x, y), enter: "pop", kind };
}

function labelFor(s: TextStyle) {
  return TEXT_PRESETS.find((p) => p.style === s)?.label ?? "Text";
}

export function newScene(n: number, preset = "studio"): Scene {
  return {
    id: uid("scene"),
    name: `Scene ${n}`,
    duration: 150,
    background: { preset, colors: [] },
    camera: { from: { x: 640, y: 360, zoom: 1 }, to: { x: 640, y: 360, zoom: 1 }, moving: false, ease: "inOut" },
    transition: { kind: "cut", duration: 15 },
    items: [],
    audio: [],
  };
}

export const EXPRESSIONS: { id: CharacterKind["expression"]; label: string; icon: string }[] = [
  { id: "neutral", label: "Neutral", icon: "😐" },
  { id: "happy", label: "Happy", icon: "😄" },
  { id: "sad", label: "Sad", icon: "😢" },
  { id: "angry", label: "Angry", icon: "😠" },
  { id: "surprised", label: "Surprised", icon: "😮" },
  { id: "thinking", label: "Thinking", icon: "🤔" },
  { id: "wink", label: "Wink", icon: "😉" },
];
