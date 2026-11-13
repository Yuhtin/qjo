import { defineConfig } from "vite";

export default defineConfig({
  // Os exemplos vêm de ../examples (importados com ?raw).
  server: { fs: { allow: [".."] } },
  build: { target: "es2022" },
});
