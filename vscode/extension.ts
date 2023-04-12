import vscode from "vscode"
import { spawn, spawnSync } from "child_process"
import { temporaryFile } from "tempy"
import fs from "fs"
import path from "path"
import which from "which"
import { Packr, Unpackr } from "msgpackr"
import os from "os"

const packr = new Packr({ useRecords: false, preserveNumericTypes: true })
const unpackr = new Unpackr({ largeBigIntToFloat: false, int64AsNumber: false, mapsAsObjects: true, useRecords: true, preserveNumericTypes: true })

class LocalPythonClient {
    readonly #p
    readonly #requestBody = temporaryFile({ extension: "msgpack" })
    readonly #responseBody = temporaryFile({ extension: "msgpack" })
    #closed: false | "success" | "error" = false

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
        this.#p.on("error", (err) => {
            vscode.window.showErrorMessage(err.message)
        })
        let errors = ""
        this.#p.stderr.on("data", (err: Buffer) => {
            const errStr = err.toString()
            if (errStr.includes("Traceback (") || this.#closed === "error") {
                // Show the error if it is a runtime error.
                vscode.window.showErrorMessage(errStr)
            } else {
                console.error(errStr)
                errors += errStr + "\n"
            }
        })
        this.#p.stdout.on("data", (data: Buffer) => {
            const status = +data.toString().trim()
            if (status === 400) {
                this.#reject(new Error(fs.readFileSync(this.#responseBody).toString()))
            } else if (status === 200) {
                this.#resolve(fs.readFileSync(this.#responseBody))
            }
        })
        this.#p.on("exit", (code) => {
            if (this.#closed) { return }
            this.#closed = "error"
            vscode.window.showErrorMessage(`Process exited: ${code}\n${errors}`)
        })
    }

    #queue = new Array<() => void>()
    request(url: string, body: Buffer | Uint8Array, resolve: (data: Buffer) => void, reject: (err: Error) => void) {
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
        this.#closed ||= "success"
        fs.rmSync(this.#requestBody, { force: true })
        fs.rmSync(this.#responseBody, { force: true })
        this.#p.kill()
    }
}

const supportedPythonVersion = [3, 6] as const
const supportedSQLiteVersion = [3, 8]

const checkPythonVersion = (filepath: string) => {
    const pythonVersionCheck = spawnSync(filepath, ["-c", `import sys; print(sys.version_info >= (${supportedPythonVersion[0]}, ${supportedPythonVersion[1]}))`]).stdout.toString()
    if (!pythonVersionCheck.includes("True")) { return false }
    const sqliteVersionCheck = spawnSync(filepath, ["-c", `import sqlite3; print(tuple(map(int, sqlite3.sqlite_version.split("."))) >= (${supportedSQLiteVersion[0]}, ${supportedSQLiteVersion[1]}))`]).stdout.toString()
    if (!sqliteVersionCheck.includes("True")) { return false }
    return true
}

const findPython = async () => {
    const [major, minor] = supportedPythonVersion

    // Microsoft store
    for (const name of [
        ...[...Array(10).keys()].map((x) => `python${major}.${x + minor}`).reverse(),
        `python${major}`,
    ]) {
        // fs.existsSync(filepath) throws permission errors so we need to try executing the binary.
        const filepath = `${os.homedir()}\\AppData\\Local\\Microsoft\\WindowsApps\\${name}.exe`
        if (!spawnSync(filepath, ["--version"]).stdout?.toString().includes("Python")) { continue }
        if (!checkPythonVersion(filepath)) { continue }
        return filepath
    }

    // Other installations
    for (const name of [
        ...[...Array(10).keys()].map((x) => `python${major}.${x + minor}`).reverse(),
        `python${major}`,
        "python",
        "py",
    ]) {
        try {
            const filepath = await which(name)  // which() doesn't find applications in the WindowsApps directory.
            if (!checkPythonVersion(filepath)) { continue }
            return filepath
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
                const pythonPath = (vscode.workspace.getConfiguration("sqlite3-editor").get<string>("pythonPath") || await findPython())
                if (!pythonPath) {
                    const msg = `Could not find a Python >=${supportedPythonVersion[0]}.${supportedPythonVersion[1]} binary compiled with SQLite >=${supportedSQLiteVersion[0]}.${supportedSQLiteVersion[1]} .Install one from https://www.python.org/ or your OS's package manager (Microsoft Store, brew, apt, etc.).`
                    vscode.window.showErrorMessage(msg)
                    throw new Error(msg)
                }
                if (uri.scheme === "file") {
                    const conn = new LocalPythonClient(pythonPath, context.asAbsolutePath("server.py"), uri.fsPath, path.dirname(uri.fsPath))
                    const watcher = fs.watch(uri.fsPath)
                    return {
                        uri,
                        unsupportedScheme: false,
                        dispose: () => {
                            conn.close()
                            watcher.removeAllListeners()
                            watcher.close()
                        },
                        conn,
                        watcher,
                    }
                } else {  // unsupportedScheme such as the diff view.
                    return {
                        uri,
                        unsupportedScheme: true,
                        dispose: () => { },
                    }
                }
            },
            async resolveCustomEditor(document, webviewPanel, token) {
                if (document.unsupportedScheme) {
                    if (document.uri.scheme === "git") {
                        let options = ""
                        try {
                            if (JSON.parse(decodeURIComponent(document.uri.query)).ref === "") {
                                options = " --staged"
                            }
                        } catch (err) { console.error(err) }
                        webviewPanel.webview.html = `The sqlite3-editor extension doesn't support the git-diff view. Use the following command instead.<br /><code>${escapeHTML(`git -c 'diff.default.textconv=echo .dump | sqlite3' diff${options} '${document.uri.fsPath}'`)}</code>`
                    } else {
                        webviewPanel.webview.html = `Unsupported file scheme: ${escapeHTML(document.uri.scheme)}`
                    }
                    return
                }
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
                    switch (url) {
                        case "/setState": {
                            const { key, value } = packr.unpack(body) as { key: string, value: unknown }
                            context.workspaceState.update(key, value)
                                .then(() => { res.send(packr.pack(null)) }, (err) => { res.status(400).send((err as Error).message) })
                            break
                        } case "/downloadState": {
                            res.send(packr.pack(Object.fromEntries(context.workspaceState.keys().map((key) => [key, context.workspaceState.get(key)]))))
                            break
                        } default:
                            document.conn.request(url, body, (body) => res.send(body), (err) => { res.status(400).send((err as Error).message) })
                            break
                    }
                }))

                document.watcher.on("change", () => {
                    webviewPanel.webview.postMessage({ type: "sqlite3-editor-server" })
                })
            },
        } /* satisfies */ as vscode.CustomReadonlyEditorProvider<vscode.Disposable & (
            | {
                uri: vscode.Uri
                unsupportedScheme: false
                conn: LocalPythonClient
                watcher: fs.FSWatcher
            }
            | {
                uri: vscode.Uri
                unsupportedScheme: true
            }
        )>, {
            supportsMultipleEditorsPerDocument: true,
            webviewOptions: {
                enableFindWidget: false,
                retainContextWhenHidden: true,
            },
        }),
    )
}

export const deactivate = () => { }

const escapeHTML = (t: string) => t
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;")
