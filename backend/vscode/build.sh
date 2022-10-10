#!/bin/sh

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

cd "$script_dir" || exit 1
rm -rf ./webview
cd ../../ui || exit 1
npm run build

cd "$script_dir" || exit 1
cp -r ../../ui/dist ./webview
npx esbuild extension.ts --outfile=extension.js --target=esnext --format=cjs --platform=node  --minify
