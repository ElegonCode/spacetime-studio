import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import VueRouter from "vue-router/vite";
import ui from "@nuxt/ui/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [
    VueRouter({
      /* options */
    }),
    vue(),
    ui({
      ui: {
        colors: {
          primary: "rose",
          neutral: "neutral",
        },
      },
    }),
  ],

  optimizeDeps: {
    force: true,
  },

  build: {
    rollupOptions: {
      onLog(level, log, handler) {
        if (
          log.code === "INVALID_ANNOTATION" &&
          log.id?.includes("@vueuse/core")
        ) {
          return;
        }

        handler(level, log);
      },
    },
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
