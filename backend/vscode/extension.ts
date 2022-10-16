import vscode from "vscode"
import sqlite3 from "better-sqlite3"
import { pack, unpack } from "msgpackr"
import fs from "fs"
import path from "path"
import os from "os"

export const activate = (context: vscode.ExtensionContext) => {
    context.subscriptions.push(
        vscode.window.registerCustomEditorProvider("sqlite3-editor.editor", {
            openCustomDocument(uri, openContext, token) {
                const readonlyConnection = sqlite3(uri.fsPath, { readonly: true })
                const readWriteConnection = sqlite3(uri.fsPath)
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
                        try {
                            const query = unpack(body as Buffer) as { query: string, params: (null | number | string | Buffer)[], mode: "r" | "w+" }

                            if (typeof query.query !== "string") { throw new Error(`Invalid arguments: ${JSON.stringify(query)}`) }
                            if (!(Array.isArray(query.params) && query.params.every((p) => p === null || typeof p === "number" || typeof p === "string" || p instanceof Buffer))) { throw new Error(`Invalid arguments: ${JSON.stringify(query)}`) }
                            if (!["r", "w+"].includes(query.mode)) { throw new Error(`Invalid arguments: ${JSON.stringify(query)}`) }

                            try {
                                const statement = (query.mode === "w+" ? document.readWriteConnection : document.readonlyConnection).prepare(query.query)
                                if (statement.reader) {
                                    res.send(pack(statement.all(...query.params)))
                                } else {
                                    statement.run(...query.params)
                                    res.send(pack(undefined))
                                }
                            } catch (err) {
                                throw new Error(`${(err as Error).message}\nQuery: ${query.query}\nParams: ${JSON.stringify(query.params)}`)
                            }
                        } catch (err) {
                            res.status(400).send((err as Error).message)
                        }
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
