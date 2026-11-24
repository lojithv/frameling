// Platform layer: the same UI runs in the Tauri desktop shell and in a plain
// browser. Desktop uses native dialogs, native files and the native (FFmpeg)
// exporter; the web build uses downloads and the WebCodecs exporter.

import { app, eng, loadProject, toast, rememberRecent, forgetRecent } from "./store.svelte";
import { exportInBrowser } from "./webexport";

export const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const FILTER = [{ name: "Frameling project", extensions: ["frameling", "json"] }];

async function tauri() {
  const [{ invoke }, dialog, event] = await Promise.all([
    import("@tauri-apps/api/core"),
    import("@tauri-apps/plugin-dialog"),
    import("@tauri-apps/api/event"),
  ]);
  return { invoke, dialog, event };
}

function safeName() {
  return (app.project?.name || "Untitled").replace(/[^\w\- ]+/g, "").trim() || "Untitled";
}

function download(blob: Blob, name: string) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
}

export async function openProject() {
  if (isDesktop) {
    const { invoke, dialog } = await tauri();
    const path = await dialog.open({ filters: FILTER, multiple: false });
    if (typeof path !== "string") return;
    await openPath(path);
    return;
  }
  const input = document.createElement("input");
  input.type = "file";
  input.accept = ".frameling,.json,application/json";
  input.onchange = async () => {
    const f = input.files?.[0];
    if (f) {
      loadProject(await f.text(), null);
      toast(`Opened ${f.name}`);
    }
  };
  input.click();
}

/** Desktop: open a project file by path (File → Open… and Recent projects). */
export async function openPath(path: string): Promise<boolean> {
  const { invoke } = await tauri();
  let json: string;
  try {
    json = await invoke<string>("read_text_file", { path });
  } catch {
    forgetRecent(path);
    toast(`Can't find ${path.split(/[\\/]/).pop()}. It was moved or deleted, so it's off the recent list.`, "error");
    return false;
  }
  if (!loadProject(json, path)) return false;
  app.fileMtime = await invoke<number>("file_mtime", { path });
  rememberRecent(path, app.project?.name ?? "");
  toast("Project opened");
  return true;
}

export async function saveProject(saveAs = false) {
  const json = eng().project_json();
  if (isDesktop) {
    const { invoke, dialog } = await tauri();
    let path = saveAs ? null : app.filePath;
    if (!path) {
      path = await dialog.save({ filters: FILTER, defaultPath: `${safeName()}.frameling` });
      if (!path) return;
    }
    await invoke("write_text_file", { path, contents: json });
    app.filePath = path;
    app.fileMtime = await invoke<number>("file_mtime", { path });
    rememberRecent(path, app.project?.name ?? "");
  } else {
    download(new Blob([json], { type: "application/json" }), `${safeName()}.frameling`);
  }
  eng().mark_saved();
  app.dirty = false;
  toast("Project saved");
}

/** Save the auto subtitles as an .srt file (for uploading next to the video). */
export async function saveSubtitles() {
  const srt = eng().captions_srt();
  if (!srt.trim()) {
    toast("No subtitles yet: add a voice line (or type a caption on a sound) first.", "error");
    return;
  }
  if (isDesktop) {
    const { invoke, dialog } = await tauri();
    const path = await dialog.save({ filters: [{ name: "Subtitles", extensions: ["srt"] }], defaultPath: `${safeName()}.srt` });
    if (!path) return;
    await invoke("write_text_file", { path, contents: srt });
  } else {
    download(new Blob([srt], { type: "text/plain" }), `${safeName()}.srt`);
  }
  toast("Subtitles saved");
}

export interface ExportSettings {
  width: number;
  height: number;
  format: "mp4" | "webm";
}

let cancelRequested = false;
export function cancelExport() {
  cancelRequested = true;
  if (isDesktop) tauri().then(({ invoke }) => invoke("cancel_export"));
}

export async function exportVideo(s: ExportSettings) {
  cancelRequested = false;
  const total = eng().total_frames();
  if (isDesktop) {
    const { invoke, dialog, event } = await tauri();
    const path = await dialog.save({
      filters: [{ name: s.format.toUpperCase(), extensions: [s.format] }],
      defaultPath: `${safeName()}.${s.format}`,
    });
    if (!path) return;
    app.exporting = { done: 0, total, label: "Rendering" };
    const un = await event.listen<[number, number]>("export-progress", (e) => {
      app.exporting = { done: e.payload[0], total: e.payload[1], label: "Rendering" };
    });
    try {
      await invoke("export_video", { project: eng().project_json(), path, width: s.width, height: s.height });
      toast(`Exported ${path}`);
    } catch (e) {
      toast(String(e), cancelRequested ? "info" : "error");
    } finally {
      un();
      app.exporting = null;
    }
    return;
  }
  app.exporting = { done: 0, total, label: "Rendering" };
  try {
    const blob = await exportInBrowser(s, (done, label) => {
      app.exporting = { done, total, label };
      return !cancelRequested;
    });
    if (blob) {
      download(blob, `${safeName()}.${blob.type.includes("webm") ? "webm" : "mp4"}`);
      toast("Video exported");
    } else {
      toast("Export cancelled");
    }
  } catch (e) {
    toast(`Export failed: ${(e as Error).message ?? e}`, "error");
  } finally {
    app.exporting = null;
  }
}

/** Load an asset pack (props, backgrounds, actions or character parts). Resolves once it is loaded. */
export function loadPackFile(): Promise<void> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json,application/json";
    input.onchange = async () => {
      const f = input.files?.[0];
      if (!f) return resolve();
      try {
        const id = eng().add_pack(await f.text());
        app.catalog = JSON.parse(eng().catalog());
        app.rev++;
        toast(`Loaded pack "${id}"`);
      } catch (e) {
        toast(`Invalid pack: ${(e as Error).message ?? e}`, "error");
      }
      resolve();
    };
    input.click();
  });
}

// ---------------------------------------------------------------- live sync with agents
// When the open file is changed by someone else (an AI agent through
// `frameling mcp`, or a script), reload it, keeping the current view.
let watching = false;
export function watchFileForAgents() {
  if (!isDesktop || watching) return;
  watching = true;
  let warned = 0;
  setInterval(async () => {
    if (!app.filePath || app.exporting) return;
    const { invoke } = await tauri();
    let m: number;
    try {
      m = await invoke<number>("file_mtime", { path: app.filePath });
    } catch {
      return;
    }
    if (!app.fileMtime || m === app.fileMtime) return;
    if (app.dirty) {
      if (warned !== m) toast("This file was changed outside the app. Save to keep your version, or reopen it.", "info");
      warned = m;
      return;
    }
    const json = await invoke<string>("read_text_file", { path: app.filePath });
    app.fileMtime = m;
    loadProject(json, app.filePath, true);
    toast("Updated from changes made by an agent");
  }, 1200);
}
