import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Vite config for a static portal build.
export default defineConfig({
  plugins: [react()],
  server: {
    host: true,
    port: 5173
  }
});
