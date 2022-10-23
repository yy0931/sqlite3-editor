import vscode from "vscode"
import { spawn } from "child_process"
import { temporaryFile } from "tempy"
import fs from "fs"

class PythonSqlite3Client {
    readonly #p
    readonly #requestBody = temporaryFile({ extension: "msgpack" })
    readonly #responseBody = temporaryFile({ extension: "msgpack" })

    #resolve!: (data: Buffer) => void
    #reject!: (message: Error) => void

    constructor(serverScriptPath: string, databasePath: string, cwd: string) {
        this.#p = spawn("python3", [
            serverScriptPath,
            "--database-filepath", databasePath,
            "--request-body-filepath", this.#requestBody,
            "--response-body-filepath", this.#responseBody,
            "--cwd", cwd,
        ])
        this.#p.stderr.on("data", (err: Buffer) => {
            console.error(err.toString())
        })
        this.#p.stdout.on("data", (data: Buffer) => {
            const status = +data.toString().trim()
            if (status === 400) {
                this.#reject(new Error(fs.readFileSync(this.#responseBody).toString()))
            } else if (status === 200) {
                this.#resolve(fs.readFileSync(this.#responseBody))
            }
        })
    }

    #queue = new Array<() => void>()
    request(url: string, body: Buffer, resolve: (data: Buffer) => void, reject: (err: Error) => void) {
        const job = () => {
            fs.writeFileSync(this.#requestBody, body)
            this.#resolve = (data) => { resolve(data); this.#queue.shift(); this.#queue[0]?.() }
            this.#reject = (err) => { reject(err); this.#queue.shift(); this.#queue[0]?.() }
            this.#p.stdin.write(url + "\n")
        }
        this.#queue.push(job)
        if (this.#queue.length === 1) {
            this.#queue[0]!()
        }
    }

    close() {
        fs.rmSync(this.#requestBody, { force: true })
        fs.rmSync(this.#responseBody, { force: true })
        this.#p.kill()
    }
}

export const activate = (context: vscode.ExtensionContext) => {
    context.subscriptions.push(
        vscode.window.registerCustomEditorProvider("sqlite3-editor.editor", {
            openCustomDocument(uri, openContext, token) {
                const conn = new PythonSqlite3Client(context.asAbsolutePath("python-sqlite-stdio/server.py"), uri.fsPath, uri.fsPath)
                const watcher = fs.watch(uri.fsPath)
                return {
                    uri,
                    dispose: () => {
                        conn.close()
                        watcher.removeAllListeners()
                        watcher.close()
                    },
                    conn,
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
                    document.conn.request(url, body as Buffer, (body) => res.send(body), (err) => { res.status(400).send((err as Error).message) })
                }))

                document.watcher.on("change", () => {
                    webviewPanel.webview.postMessage({ type: "sqlite3-editor-server" })
                })
            },
        } as vscode.CustomReadonlyEditorProvider<vscode.Disposable & {
            uri: vscode.Uri
            conn: PythonSqlite3Client,
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
