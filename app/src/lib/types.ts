// TypeScript mirror of the Rust document model (crates/core/src/model.rs).

export type Id = string;
export type Ease = "linear" | "inOut" | "in" | "out" | "back";
export type Effect = "none" | "fade" | "pop" | "slideLeft" | "slideRight" | "slideUp" | "slideDown" | "bounce" | "zoom" | "spin";
export type Font = "sans" | "heavy" | "rounded" | "mono" | "serif";
export type Reveal = "none" | "typewriter" | "popLetters" | "popWords" | "riseWords" | "bounce" | "fade";
export type Emphasis = "none" | "wave" | "pulse" | "shake";
export type CaptionStyle = "pop" | "boxed" | "pill" | "karaoke" | "clean";

/** Auto subtitles made from the voice lines (crates/core/src/captions.rs). */
export interface Captions {
  enabled: boolean;
  style: CaptionStyle;
  font: Font;
  /** Letter size at a 720 px short side. */
  size: number;
  color: string;
  highlight: string;
  stroke: string;
  strokeWidth: number;
  boxColor: string;
  /** Caption centre, 0 = top, 1 = bottom. */
  y: number;
  words: number;
  uppercase: boolean;
  pop: boolean;
}
export type TransitionKind = "cut" | "fade" | "fadeBlack" | "slide" | "zoom";
export type HairStyle = "bald" | "short" | "spiky" | "long" | "bun" | "curly";
export type Build = "slim" | "regular" | "broad";
export type Accessory = "none" | "glasses" | "tie" | "cap" | "bow";
export type Expression = "neutral" | "happy" | "sad" | "angry" | "surprised" | "thinking" | "wink" | "worried" | "confused";
export type FigureStyle = "cartoon" | "stick";
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
  /** Copies of pack parts the project's characters wear. */
  parts?: Record<Id, unknown>;
  /** Poses saved in this project. */
  poses?: Record<Id, { id: string; name: string; category: string; tags: string[]; pose: Record<string, number> }>;
  /** Animations made in this project (chained from poses). */
  actions?: Record<Id, { id: string; name: string; duration: number; loop: boolean; keys: unknown[] }>;
  /** Auto subtitles (absent = defaults, off). */
  captions?: Captions;
  /** Background music across the whole movie. */
  music?: Music | null;
}

/** Background music (crates/core/src/model.rs `Music`). */
export interface Music {
  media: Id;
  volume: number;
  /** Volume multiplier while voices play (1 = no ducking). */
  duck: number;
  fadeIn: number;
  fadeOut: number;
  looped: boolean;
  /** Global frame it starts at. */
  start: number;
  trimStart: number;
}

/** A built-in sound effect or music loop (crates/core/src/sfx.rs). */
export interface SoundInfo { id: string; name: string; kind: "effect" | "music"; category: string; secs: number; looped: boolean }

export interface Scene {
  id: Id;
  name: string;
  duration: number;
  background: { preset: string; colors: string[] };
  camera: Camera;
  transition: { kind: TransitionKind; duration: number };
  items: Item[];
  audio: AudioClip[];
  /** Keyframe tracks for the scene camera (camera.x, camera.y, camera.zoom). */
  tracks?: Tracks;
}

export interface CamKey { x: number; y: number; zoom: number }
export interface Camera { from: CamKey; to: CamKey; moving: boolean; ease: Ease }

/** Keyframe interpolation: how a key eases into the next one. */
export type Interp =
  | "constant" | "linear" | "easeIn" | "easeOut" | "easeInOut" | "back" | "elastic" | "bounce"
  | { bezier: [number, number, number, number] };
/** One key. `value` is a number, "#rrggbb", bool, enum name or outfit depending on the path. */
export interface Key { frame: number; value: unknown; interp?: Interp }
export interface Track { keys: Key[] }
/** Tracks by path: transform.x, bone.forearm_l.rot, look.skin, outfit, expression, camera.zoom, ... */
export type Tracks = Record<string, Track>;
/** A costume: the look fields it changes. */
export type LookPatch = Partial<Look>;

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
  /** "stick": line-drawn figure; `top` is the line colour, `skin` the head fill. */
  style?: FigureStyle;
  /** Stick figures: wear a dress in the `accent` colour. */
  dress?: boolean;
  /** Face parts (cartoon figures). */
  eyes?: "round" | "dots" | "wide" | "sleepy" | "lashes" | "narrow";
  brows?: "normal" | "thin" | "thick" | "arched" | "none";
  nose?: "button" | "dot" | "round" | "long" | "none";
  facialHair?: "none" | "stubble" | "moustache" | "goatee" | "beard" | "fullBeard";
  /** Wearable parts from part packs, with their colour overrides. */
  wear?: { part: string; colors?: string[] }[];
  /** Lip-sync mouth set (cartoon figures; absent = classic). */
  mouth?: MouthSet;
}

/** One entry of the character parts catalogue. */
export interface PartInfo {
  id: string;
  name: string;
  category: string;
  /** "builtin", or the part pack it came from. */
  pack: string;
  tags: string[];
  styles: string[];
  /** Default colours of a pack part's own slots. */
  colors: string[];
  /** The look colour a built-in part uses ("hair", "accent"). */
  colorField?: "skin" | "hair" | "top" | "bottom" | "shoes" | "accent";
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
  /** Named costumes the `outfit` track can switch to. */
  outfits?: Record<string, LookPatch>;
  /** Poses held over the animation, with blends in and out. */
  poses?: PoseStrip[];
  /** Turnaround view. */
  view?: View;
  /** Face the way it walks. */
  autoTurn?: boolean;
  /** How the mouth follows voice lines (absent = shapes). */
  lipSync?: LipSync;
}

/** Lip-sync mouth shapes (crates/core/src/lipsync.rs). */
export type Viseme = "rest" | "mbp" | "ee" | "eh" | "ai" | "o" | "u" | "fv" | "l";
export type LipSync = "shapes" | "simple" | "off";
export type MouthSet = "classic" | "lips";

export type View = "front" | "frontRight" | "right" | "backRight" | "back" | "backLeft" | "left" | "frontLeft";

export type PoseMask = "full" | "upper" | "lower" | "armL" | "armR" | "head";
export interface PoseStrip {
  pose: string;
  start: number;
  end: number;
  blendIn: number;
  blendOut: number;
  mask: PoseMask;
  mirror: boolean;
  breathe: boolean;
}
export interface PoseInfo { id: string; name: string; category: string; tags: string[]; pack?: string }
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
  font?: Font;
  stroke?: string;
  /** Outline thickness, stage px (0 = none). */
  strokeWidth?: number;
  uppercase?: boolean;
  reveal?: Reveal;
  revealOut?: Reveal;
  /** Letters (or words) per second. */
  revealSpeed?: number;
  emphasis?: Emphasis;
  /** Ignores the camera and draws on top. */
  pinned?: boolean;
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
  /** Keyframe tracks; keyed values override the fields above. */
  tracks?: Tracks;
}

export interface AudioClip {
  id: Id;
  media: Id;
  start: number;
  volume: number;
  speaker: Id | null;
  trimStart: number;
  length: number | null;
  /** Subtitle text: absent = the typed line, "" = no subtitle. */
  caption?: string | null;
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

export interface Voice { id: string; name: string; language: string; engine: "supertonic" | "system"; credit: string }

export interface Rect { x: number; y: number; w: number; h: number }
export interface ItemHit { id: Id; bounds: Rect; visible: boolean }

export interface AudioEvent {
  media: Id;
  at: number;
  trim: number;
  frames: number;
  volume: number;
  looped?: boolean;
  fadeIn?: number;
  fadeOut?: number;
  /** Global frame spans where voices play; the gain ducks to duckLevel there. */
  ducks?: [number, number][];
  duckLevel?: number;
  duckRamp?: number;
}

export interface CatalogEntry { id: string; name: string; category: string; colors: string[]; pack: string }
export interface Catalog {
  props: CatalogEntry[];
  backgrounds: CatalogEntry[];
  actions: { id: string; name: string; loop: boolean; duration: number }[];
  packs: { id: string; name: string }[];
  poses: PoseInfo[];
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
  | { op: "addParts"; parts: unknown[] }
  | { op: "moveKeys"; scene: Id; item?: Id | null; from: number; to: number; paths?: string[] | null }
  | { op: "addPose"; pose: { id: string; name: string; category: string; tags: string[]; pose: unknown } }
  | { op: "addAction"; action: unknown }
  | { op: "autoKey"; scene: Id; item: Item; frame: number; auto?: boolean }
  | { op: "setKeys"; scene: Id; item?: Id | null; path: string; keys: Key[]; replace?: boolean }
  | { op: "deleteKeys"; scene: Id; item?: Id | null; path?: string | null; frames?: number[] | null; from?: number | null; to?: number | null }
  | { op: "setInterpolation"; scene: Id; item?: Id | null; path: string; frames?: number[] | null; interp: Interp }
  | { op: "defineOutfit"; scene: Id; item: Id; name: string; outfit: LookPatch | null }
  | { op: "setCaptions"; captions: Captions }
  | { op: "setMusic"; music: Music | null }
  | { op: "addBuiltinMedia"; sound: string }
  | { op: "batch"; commands: Command[] };
