import { defineConfig } from "vite"
import preact from "@preact/preset-vite"

export default defineConfig({
    plugins: [preact()],
    build: {
        target: "esnext"
    },
    esbuild: {
        logOverride: { 'this-is-undefined-in-esm': 'silent' }
    },
})
