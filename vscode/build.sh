#!/bin/sh

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Build ../ui/ and copy the bundle ./webview
cd "$script_dir" || exit 1
rm -rf ./webview
cd ../ui || exit 1
npm run build
cd "$script_dir" || exit 1
cp -r ../ui/dist ./webview

# Build ./extension.ts
npx esbuild extension.ts --bundle --outfile=extension.js --target=esnext --format=cjs --platform=node --minify --external:vscode
