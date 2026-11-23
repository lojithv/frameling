fn main() {
    // tauri-build embeds icons/icon.ico as the exe's icon resource, but cargo
    // does not always notice when the icon files change, which left builds with
    // a stale (or missing) icon. Rebuild the resource whenever they change.
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build()
}
