import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { installer } from "./tools/installer.mjs";

// Relative base so the built site works from any folder or sub-path.
export default defineConfig({
  base: "./",
  plugins: [react(), installer("public/download")],
  server: { port: 5180 },
  preview: { port: 5181 },
  build: {
    target: "es2022",
    sourcemap: false,
  },
});
