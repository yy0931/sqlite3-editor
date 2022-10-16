import vscode from "vscode"
import sqlite3 from "sqlite3"
import { pack, unpack } from "msgpackr"
import fs from "fs"
import path from "path"
import os from "os"

const query = (req: { body: Uint8Array }, readonlyConnection: sqlite3.Database, readWriteConnection: sqlite3.Database) => new Promise<Buffer>((resolve, reject) => {
    const query = unpack(req.body) as { query: string, params: (null | number | string | Uint8Array)[], mode: "r" | "w+" }

    if (typeof query.query !== "string") { reject(new Error(`Invalid argument: ${JSON.stringify(query)}`)); return }
    if (!(Array.isArray(query.params) && query.params.every((p) => p === null || typeof p === "number" || typeof p === "string" || p instanceof Uint8Array))) { reject(new Error(`Invalid argument: ${JSON.stringify(query)}`)); return }
    if (!["r", "w+"].includes(query.mode)) { reject(new Error(`Invalid argument: ${JSON.stringify(query)}`)); return }

    (query.mode === "w+" ? readWriteConnection : readonlyConnection).all(query.query, query.params, (err, rows) => {
        if (err !== null) { reject(new Error(`${err.message}\nQuery: ${query.query}\nParams: ${JSON.stringify(query.params)}`)); return }
        resolve(pack(rows))
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

                context.subscriptions.push(webviewPanel.webview.onDidReceiveMessage(({ requestId, path: url, body }: { requestId: number, path: string, body: Uint8Array }) => {
                    const res = {
                        send: (buf: Buffer) => { webviewPanel.webview.postMessage({ type: "sqlite3-editor-server", requestId, body: new Uint8Array(buf) }) },
                        status: (code: 400) => ({ send: (text: string) => { webviewPanel.webview.postMessage({ type: "sqlite3-editor-server", requestId, err: text }) } })
                    }

                    const resolveFilepath = (filepath: string) =>
                        path.isAbsolute(filepath) ? vscode.Uri.file(filepath) :
                            vscode.Uri.joinPath(vscode.workspace.getWorkspaceFolder(document.uri)?.uri ?? vscode.Uri.file(os.homedir()), filepath)

                    if (url === `/query`) {
                        query({ body }, document.readonlyConnection, document.readWriteConnection)
                            .then((body) => res.send(body))
                            .catch((err) => { res.status(400).send(err.message) })
                    } else if (url === "/import") {
                        const { filepath } = unpack(body as Uint8Array) as { filepath: string }
                        vscode.workspace.fs.readFile(resolveFilepath(filepath))
                            .then((buf) => { res.send(pack(buf)) }, (err) => { res.status(400).send(err.message) })
                    } else if (url === "/export") {
                        const { filepath, data } = unpack(body as Uint8Array) as { filepath: string, data: Buffer }
                        vscode.workspace.fs.writeFile(resolveFilepath(filepath), data)
                            .then(() => { res.send(pack(null)) }, (err) => { res.status(400).send(err.message) })
                    }
                }))

                document.watcher.on("change", () => {
                    webviewPanel.webview.postMessage({ type: "sqlite3-editor-server" })
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
