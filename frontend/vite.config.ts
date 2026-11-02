import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";

// Build the outl desktop client for a plain browser. The desktop crate is the
// most complete client (full command surface, palette, settings). Its source
// is consumed verbatim from the vendored checkout; the only change is that
// every `@tauri-apps/*` import is aliased to a browser shim that talks to the
// outl-server over HTTP + WebSocket instead of Tauri IPC.
const desktop = fileURLToPath(
  new URL("../vendor/outl/crates/outl-desktop", import.meta.url),
);
const shared = fileURLToPath(
  new URL("../vendor/outl/crates/outl-frontend-shared/src", import.meta.url),
);

export default defineConfig({
  root: desktop,
  // Relative base so the built index.html references assets as `./assets/…`.
  // In an HA ingress panel the UI loads in an iframe whose document URL is
  // `/hassio_ingress/{token}/`; relative refs resolve against that, so the
  // bundle loads from the addon (the proxy strips the prefix upstream).
  base: "./",
  plugins: [solid(), tailwindcss()],
  resolve: {
    alias: {
      // The shared outl frontend library.
      "@outl/shared": shared,
      // Tauri IPC -> HTTP.
      "@tauri-apps/api/core": fileURLToPath(new URL("./shims/core.ts", import.meta.url)),
      "@tauri-apps/api/event": fileURLToPath(new URL("./shims/event.ts", import.meta.url)),
      "@tauri-apps/api/window": fileURLToPath(new URL("./shims/api-window.ts", import.meta.url)),
      "@tauri-apps/api/webview": fileURLToPath(new URL("./shims/api-webview.ts", import.meta.url)),
      "@tauri-apps/api/mocks": fileURLToPath(new URL("./shims/api-mocks.ts", import.meta.url)),
      // Tauri plugins -> browser equivalents.
      "@tauri-apps/plugin-opener": fileURLToPath(new URL("./shims/plugin-opener.ts", import.meta.url)),
      "@tauri-apps/plugin-dialog": fileURLToPath(new URL("./shims/plugin-dialog.ts", import.meta.url)),
      "@tauri-apps/plugin-os": fileURLToPath(new URL("./shims/plugin-os.ts", import.meta.url)),
      "@tauri-apps/plugin-log": fileURLToPath(new URL("./shims/plugin-log.ts", import.meta.url)),
      "@tauri-apps/plugin-clipboard-manager": fileURLToPath(
        new URL("./shims/plugin-clipboard-manager.ts", import.meta.url),
      ),
    },
  },
  build: {
    outDir: fileURLToPath(new URL("./dist", import.meta.url)),
    emptyOutDir: true,
    // The server serves the bundle; no need for hashed asset names to be
    // stable across a single build, but keep them for cache-busting.
    assetsDir: "assets",
    target: "es2022",
    sourcemap: false,
  },
  // `import.meta.env.DEV` is false in a production build, so the dev-only
  // `?mock=1` branch in index.tsx is eliminated and the mock module is never
  // bundled.
  define: {
    "import.meta.env.VITE_OUTL_API": JSON.stringify(""),
  },
});
