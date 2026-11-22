// Builds the Rust engine to WebAssembly and generates JS bindings into src/engine/.
// Needs: rustup target add wasm32-unknown-unknown && cargo install wasm-bindgen-cli --version 0.2.130
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const run = (cmd) => execSync(cmd, { cwd: root, stdio: "inherit" });

run("cargo build --release --target wasm32-unknown-unknown -p frameling-wasm");
run(
  "wasm-bindgen --target web --no-typescript --out-dir app/src/engine --out-name frameling " +
    "target/wasm32-unknown-unknown/release/frameling_wasm.wasm"
);
console.log("wasm engine ready in app/src/engine/");
