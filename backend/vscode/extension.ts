import vscode from "vscode"
import sqlite3 from "sqlite3"
import { pack, unpack } from "msgpackr"
import fs from "fs"

const query = (req: { body: Uint8Array }, readonlyConnection: sqlite3.Database, readWriteConnection: sqlite3.Database) => new Promise<Uint8Array>((resolve, reject) => {
    const query = unpack(req.body) as { query: string, params: (null | number | string | Buffer)[], mode: "r" | "w+" }

    if (typeof query.query !== "string") { reject(new Error(`Invalid search params: ${JSON.stringify(query)}`)); return }
    if (!(Array.isArray(query.params) && query.params.every((p) => p === null || typeof p === "number" || typeof p === "string" || p instanceof Buffer))) { reject(new Error(`Invalid search params: ${JSON.stringify(query)}`)); return }
    if (!["r", "w+"].includes(query.mode)) { reject(new Error(`Invalid search params: ${JSON.stringify(query)}`)); return }

    (query.mode === "w+" ? readWriteConnection : readonlyConnection).all(query.query, query.params, (err, rows) => {
        if (err !== null) { reject(new Error(`${err.message}\nQuery: ${query.query}\nParams: ${JSON.stringify(query.params)}`)); return }
        resolve(new Uint8Array(pack(rows)))
    })
})

export const activate = (context: vscode.ExtensionContext) => {
    context.subscriptions.push(
        vscode.window.registerCustomEditorProvider("sqlite3-editor.editor", {
            openCustomDocument(uri, openContext, token) {
                const readonlyConnection = new sqlite3.Database(uri.fsPath, sqlite3.OPEN_READONLY)
                const readWriteConnection = new sqlite3.Database(uri.fsPath)
                const watcher = fs.watch(uri.fsPath)
                return {
                    uri,
                    dispose: () => {
                        readonlyConnection.close()
                        readWriteConnection.close()
                        watcher.removeAllListeners()
                        watcher.close()
                    },
                    readonlyConnection,
                    readWriteConnection,
                    watcher,
                }
            },
            async resolveCustomEditor(document, webviewPanel, token) {
                webviewPanel.webview.options = {
                    enableScripts: true,
                    localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "webview")],
                }
                webviewPanel.webview.html = (await vscode.workspace.fs.readFile(vscode.Uri.joinPath(context.extensionUri, "webview", "index.html"))).toString()
                    .replace(/((?:src|href)=")\/([^"]+)(")/g, (_, m1, m2, m3) => m1 + webviewPanel.webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, "webview", m2)).toString() + m3)

                context.subscriptions.push(webviewPanel.webview.onDidReceiveMessage(({ requestId, body }: { requestId: number, body: Uint8Array }) => {
                    query({ body }, document.readonlyConnection, document.readWriteConnection)
                        .then((body) => webviewPanel.webview.postMessage({ requestId, body }))
                        .catch((err: Error) => {
                            webviewPanel.webview.postMessage({ requestId, err: err.message })
                        })
                }))

                document.watcher.on("change", () => {
                    webviewPanel.webview.postMessage({})
                })
            },
        } as vscode.CustomReadonlyEditorProvider<vscode.Disposable & {
            uri: vscode.Uri
            readonlyConnection: sqlite3.Database
            readWriteConnection: sqlite3.Database
            watcher: fs.FSWatcher
        }>, {
            supportsMultipleEditorsPerDocument: true,
            webviewOptions: {
                enableFindWidget: true,
                retainContextWhenHidden: true,
            },
        }),
    )
}

export const deactivate = () => { }
