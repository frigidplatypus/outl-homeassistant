import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vitest/config";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  resolve: {
    alias: {
      // One copy of the Tauri bridge for the client AND for `@outl/shared`.
      // Without this, bun resolves a second `@tauri-apps/api` for the shared
      // package, so a client test that mocks `@tauri-apps/api/core` or
      // `/event` does not reach the shared module's import of it: the real
      // `invoke` runs and the test fails with the host error instead
      // (the capture-inbox drain test was red for exactly that, before it
      // moved into `@outl/shared/capture`).
      "@tauri-apps/api": fileURLToPath(
        new URL("./node_modules/@tauri-apps/api", import.meta.url),
      ),
      "@outl/shared": fileURLToPath(
        new URL("../outl-frontend-shared/src", import.meta.url),
      ),
    },
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.{ts,tsx}"],
    globals: false,
  },
});
