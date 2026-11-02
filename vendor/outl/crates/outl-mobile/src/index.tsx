/* @refresh reload */
import { render } from "solid-js/web";
import { attachConsole } from "@tauri-apps/plugin-log";
import App from "./App";
import "./styles.css";

// Forward Rust logs (the `Webview` target of tauri-plugin-log) into the browser
// console, so backend `log::` lines show up in Safari Web Inspector alongside
// the frontend's own `console.*`. Best-effort: outside a Tauri webview it no-ops.
void attachConsole().catch(() => {});

const mount = () => render(() => <App />, document.getElementById("root") as HTMLElement);

// `?mock=1` in `bun run dev` swaps the Tauri backend for an in-memory
// workspace so the UI runs in a plain browser. Dev builds only: the
// branch, and the mock with it, is dropped from production bundles.
if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock")) {
  void import("@outl/shared/mock").then((m) => {
    if (m.wantsMock()) m.installMockBackend();
    mount();
  });
} else {
  mount();
}
