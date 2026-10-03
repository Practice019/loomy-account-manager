import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"

// Tauri 期望一个固定端口的 dev server，且不能在失败时自动换端口 ——
// 换端口的话 Tauri 窗口会连到一个不存在的地方，表现为白屏。
export default defineConfig({
  plugins: [react()],
  // 防止 vite 把 Tauri 的 IPC 当需要转发的路径
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // src-tauri 里的改动由 cargo 自己监听，vite 别管
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // Tauri 的 WebView 支持较新的语法，不必为老浏览器降级
    target: "es2021",
    minify: "esbuild",
    sourcemap: false,
    // 只打包应用本身。`measure.html`（布局测量台）是开发工具，
    // **故意不进** 构建入口 —— 它是用来在 dev server 里手动打开的，
    // 打进 dist 只会让安装包里多带一份无用的模拟数据。
  },
})
