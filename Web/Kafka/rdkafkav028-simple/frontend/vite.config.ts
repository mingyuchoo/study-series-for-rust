import { defineConfig, loadEnv } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig(({ mode }) => {
  const { WEB_ADDR = "127.0.0.1:3000" } = loadEnv(mode, ".", "WEB_ADDR");
  const apiAddress = WEB_ADDR.replace(/^(0\.0\.0\.0|\[::\]):/, "127.0.0.1:");
  return {
    plugins: [solid()],
    server: {
      port: 5173,
      strictPort: true,
      proxy: { "/api": `http://${apiAddress}` },
    },
  };
});
