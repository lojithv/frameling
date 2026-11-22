// TypeScript mirror of the Rust document model (crates/core/src/model.rs).

export type Id = string;
export type Ease = "linear" | "inOut" | "in" | "out" | "back";
export type Effect = "none" | "fade" | "pop" | "slideLeft" | "slideRight" | "slideUp";
export type TransitionKind = "cut" | "fade" | "fadeBlack" | "slide" | "zoom";
export type HairStyle = "bald" | "short" | "spiky" | "long" | "bun" | "curly";
export type Build = "slim" | "regular" | "broad";
export type Accessory = "none" | "glasses" | "tie" | "cap" | "bow";
export type Expression = "neutral" | "happy" | "sad" | "angry" | "surprised" | "thinking" | "wink";
export type TextStyle = "bubble" | "thought" | "caption" | "title" | "plain";
export type Tail = "bottomLeft" | "bottomRight" | "none";

export interface Project {
  format: string;
  version: number;
  name: string;
  width: number;
  height: number;
  fps: number;
  scenes: Scene[];
  media: Record<Id, Media>;
}

export interface Scene {
  id: Id;
  name: string;
  duration: number;
  background: { preset: string; colors: string[] };
  camera: Camera;
  transition: { kind: TransitionKind; duration: number };
  items: Item[];
  audio: AudioClip[];
}

export interface CamKey { x: number; y: number; zoom: number }
export interface Camera { from: CamKey; to: CamKey; moving: boolean; ease: Ease }

export interface Motion { x: number; y: number; start: number; end: number; ease: Ease }

export interface Look {
  skin: string;
  hair: string;
  hairStyle: HairStyle;
  top: string;
  bottom: string;
  shoes: string;
  build: Build;
  accessory: Accessory;
  accent: string;
}

export interface CharacterKind {
  type: "character";
  look: Look;
  action: string;
  expression: Expression;
  speed: number;
  autoWalk: boolean;
  autoTalk: boolean;
  voice: string | null;
  voiceSpeed: number;
}
export interface PropKind { type: "prop"; prop: string; colors: string[] }
export interface TextKind {
  type: "text";
  text: string;
  style: TextStyle;
  size: number;
  color: string;
  fill: string;
  tail: Tail;
  width: number;
  typewriter: number;
}
export type ItemKind = CharacterKind | PropKind | TextKind;

export interface Item {
  id: Id;
  name: string;
  start: number;
  end: number | null;
  x: number;
  y: number;
  scale: number;
  rotation: number;
  flip: boolean;
  opacity: number;
  enter: Effect;
  exit: Effect;
  motion: Motion | null;
  locked: boolean;
  kind: ItemKind;
}

export interface AudioClip {
  id: Id;
  media: Id;
  start: number;
  volume: number;
  speaker: Id | null;
  trimStart: number;
  length: number | null;
}

export interface Media {
  id: Id;
  name: string;
  mime: string;
  data: string;
  durationFrames: number;
  envelope: number[];
  source: MediaSource | null;
}

export type MediaSource =
  | { kind: "tts"; text: string; voice: string; speed: number }
  | { kind: "recording" }
  | { kind: "file" };

export interface Voice { id: string; name: string; language: string; engine: "piper" | "system"; credit: string }

export interface Rect { x: number; y: number; w: number; h: number }
export interface ItemHit { id: Id; bounds: Rect; visible: boolean }

export interface AudioEvent { media: Id; at: number; trim: number; frames: number; volume: number }

export interface CatalogEntry { id: string; name: string; category: string; colors: string[]; pack: string }
export interface Catalog {
  props: CatalogEntry[];
  backgrounds: CatalogEntry[];
  actions: { id: string; name: string; loop: boolean; duration: number }[];
  packs: { id: string; name: string }[];
}

export type Command =
  | { op: "setMeta"; name?: string; fps?: number; width?: number; height?: number }
  | { op: "addScene"; scene: Scene; index?: number }
  | { op: "removeScene"; id: Id }
  | { op: "moveScene"; id: Id; to: number }
  | { op: "updateScene"; scene: Scene }
  | { op: "addItem"; scene: Id; item: Item; index?: number }
  | { op: "removeItem"; scene: Id; id: Id }
  | { op: "updateItem"; scene: Id; item: Item }
  | { op: "moveItem"; scene: Id; id: Id; to: number }
  | { op: "addMedia"; media: Media }
  | { op: "removeMedia"; id: Id }
  | { op: "addAudio"; scene: Id; clip: AudioClip }
  | { op: "updateAudio"; scene: Id; clip: AudioClip }
  | { op: "removeAudio"; scene: Id; id: Id }
  | { op: "addLine"; scene: Id; clipId: Id; media: Media; speaker: Id | null; start: number | null }
  | { op: "batch"; commands: Command[] };
