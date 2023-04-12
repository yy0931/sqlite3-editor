$script_dir = (Get-Location).Path

# Build ../ui/ and copy the bundle ./webview
cd "$script_dir"
if (Test-Path "./webview") {
  Remove-Item -Path "./webview" -Recurse -Force
}
cd "../ui"
npm run _build
cd "$script_dir"
Copy-Item "../ui/dist" "./webview" -Recurse

# Build ./extension.ts
npx esbuild extension.ts --bundle --outfile=extension.js --target=esnext --format=cjs --platform=node --minify --external:vscode
