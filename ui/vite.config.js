import { defineConfig } from "vite"
import preact from "@preact/preset-vite"
import license from "rollup-plugin-license"
import path from "path"

export default defineConfig({
    plugins: [
        preact(),

        // https://github.com/vitejs/vite/discussions/7722#discussioncomment-4007436
        license({
            thirdParty: {
                output: path.resolve(__dirname, "./dist/assets/vendor.LICENSE.txt"),
            },
        }),
    ],
    build: {
        target: "esnext",
    },
    esbuild: {
        logOverride: { 'this-is-undefined-in-esm': 'silent' },

        // https://github.com/vitejs/vite/discussions/7722#discussioncomment-4007436
        banner: '/*! licenses: /assets/vendor.LICENSE.txt */',
        legalComments: 'none',
    },
})
