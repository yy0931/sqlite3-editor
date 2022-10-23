import vscode from "vscode"
import { spawn, spawnSync } from "child_process"
import { temporaryFile } from "tempy"
import fs from "fs"
import { loadPyodide } from "pyodide"
import path from "path"
import which from "which"

class LocalPythonClient {
    readonly #p
    readonly #requestBody = temporaryFile({ extension: "msgpack" })
    readonly #responseBody = temporaryFile({ extension: "msgpack" })

    #resolve!: (data: Buffer) => void
    #reject!: (message: Error) => void

    constructor(pythonPath: string, serverScriptPath: string, databasePath: string, cwd: string) {
        this.#p = spawn(pythonPath, [
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

class PyodideClient {
    readonly #pyodide
    readonly #requestBody = "/home/pyodide/request_body.msgpack"
    readonly #responseBody = "/home/pyodide/response_body.msgpack"

    constructor(serverScriptPath: string, databasePath: string, cwd: string) {
        const serverScriptDir = path.dirname(serverScriptPath)
        this.#pyodide = loadPyodide({ indexURL: path.join(path.dirname(serverScriptPath), "node_modules/pyodide") }).then((pyodide) => {
            for (const filename of fs.readdirSync(serverScriptDir).filter((name) => name.endsWith(".py"))) {
                pyodide.FS.writeFile(path.join("/home/pyodide", filename), fs.readFileSync(path.join(serverScriptDir, filename)))
            }
            pyodide.FS.mkdir("/mnt")
            pyodide.FS.mount(pyodide.FS.filesystems.NODEFS, { root: path.dirname(databasePath) }, "/mnt")

            pyodide.runPython(`\
from server import Server
server = Server(${JSON.stringify(path.join("/mnt", path.basename(databasePath)))}, ${JSON.stringify(this.#requestBody)}, ${JSON.stringify(this.#responseBody)}, ${JSON.stringify(cwd)})
`)

            return pyodide
        })
        this.#pyodide.catch((err) => {
            console.error(err)
        })
    }

    request(url: string, body: Buffer, resolve: (data: Uint8Array) => void, reject: (err: Error) => void) {
        this.#pyodide.then((pyodide) => {
            // Run synchronously
            pyodide.FS.writeFile(this.#requestBody, body)
            const status: number = pyodide.runPython(`server.handle(${JSON.stringify(url)})`)
            if (status === 200) {
                resolve(pyodide.FS.readFile(this.#responseBody, { encoding: "binary" }))
            } else {
                reject(new Error(pyodide.FS.readFile(this.#responseBody, { encoding: "utf8" })))
            }
        }).catch((err) => {
            reject(err)
        })
    }

    close() {
        this.#pyodide.then((pyodide) => {
            pyodide.FS.unmount("/mnt")
            pyodide.globals.destroy()
            // TODO: how can I destroy the pyodide process?
        })
        fs.rmSync(this.#requestBody, { force: true })
        fs.rmSync(this.#responseBody, { force: true })
    }
}

/** Find a Python 3.6+ binary */
const findPython = async () => {
    for (const name of [...[...Array(10).keys()].map((x) => `python3.${x + 6}`).reverse(), "python3", "python", "py"]) {
        try {
            const filepath = await which(name)
            const out = spawnSync(filepath, ["-c", "import sys; print(sys.version_info >= (3, 6))"]).stdout.toString()
            if (out.includes("True")) {
                return filepath
            }
        } catch (err) {
            if ((err as any).code !== "ENOENT") {
                console.error(err)
            }
        }
    }
    return null
}

export const activate = (context: vscode.ExtensionContext) => {
    context.subscriptions.push(
        vscode.window.registerCustomEditorProvider("sqlite3-editor.editor", {
            async openCustomDocument(uri, openContext, token) {
                const pythonPath =
                    vscode.workspace.getConfiguration("sqlite3-editor").get<boolean>("alwaysUsePyodide") ? "" :
                        (vscode.workspace.getConfiguration("sqlite3-editor").get<string>("pythonPath") || await findPython())
                const conn = pythonPath ?
                    new LocalPythonClient(pythonPath, context.asAbsolutePath("server.py"), uri.fsPath, path.dirname(uri.fsPath)) :
                    new PyodideClient(context.asAbsolutePath("server.py"), uri.fsPath, path.dirname(uri.fsPath))
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
                        send: (buf: Buffer | Uint8Array) => { webviewPanel.webview.postMessage({ type: "sqlite3-editor-server", requestId, body: new Uint8Array(buf) }) },
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
            conn: PyodideClient | LocalPythonClient,
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
