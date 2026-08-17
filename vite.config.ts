import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri controla o processo; o Vite não deve limpar o terminal nem trocar de porta.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // WebView2 no Windows 10/11 é Chromium recente; não precisamos transpilar longe.
    target: "chrome110",
    sourcemap: false,
  },
});
