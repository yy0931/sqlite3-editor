import express from "express"
import sqlite3 from "better-sqlite3"
import { Packr, Unpackr } from "msgpackr"
import fs from "fs"
import cors from "cors"
import path from "path"

const packr = new Packr({ useRecords: false, preserveNumericTypes: true })
const unpackr = new Unpackr({ largeBigIntToFloat: false, int64AsNumber: false, mapsAsObjects: true, useRecords: true, preserveNumericTypes: true })

const find_widget_regexp = (text: string, pattern: string, wholeWord: 0n | 1n, caseSensitive: 0n | 1n) => {
    try {
        return new RegExp(wholeWord ? `\\b(?:${pattern})\\b` : pattern, caseSensitive ? "" : "i").test(text) ? 1n : 0n
    } catch (err) {
        if (err instanceof SyntaxError) { return 0n }
        throw err
    }
}

class Server {
    private readonly readonlyConnection
    private readonly readWriteConnection

    constructor(databaseFilepath: string) {
        this.readonlyConnection = sqlite3(databaseFilepath, { readonly: true })
        this.readWriteConnection = sqlite3(databaseFilepath)

        this.readonlyConnection.defaultSafeIntegers()
        this.readWriteConnection.defaultSafeIntegers()

        this.readonlyConnection.function("find_widget_regexp", { deterministic: true, varargs: false, safeIntegers: true }, find_widget_regexp as any)
        this.readWriteConnection.function("find_widget_regexp", { deterministic: true, varargs: false, safeIntegers: true }, find_widget_regexp as any)
    }

    handle(requestBodyBuffer: Buffer): [200, Buffer] | [400, string] {
        try {
            const requestBody = unpackr.unpack(requestBodyBuffer) as { query: string, params: (null | bigint | number | string | Buffer)[], mode: "r" | "w+" }
            if (typeof requestBody.query !== "string") { throw new Error(`Invalid arguments`) }
            if (!(Array.isArray(requestBody.params) && requestBody.params.every((p) => p === null || typeof p === "number" || typeof p === "bigint" || typeof p === "string" || p instanceof Buffer))) { throw new Error(`Invalid arguments`) }
            if (!["r", "w+"].includes(requestBody.mode)) { throw new Error(`Invalid arguments`) }

            try {
                if (requestBody.mode === "w+") {
                    // read-write
                    this.readWriteConnection.prepare(requestBody.query).run(...requestBody.params)
                    return [200, packr.pack(undefined)]
                } else {
                    // read-only
                    const statement = this.readonlyConnection.prepare(requestBody.query)
                    if (statement.reader) {
                        const columns = (statement.columns() as { name: string, column: string | null, table: string | null, database: string | null, type: string | null }[]).map(({ name }) => name)
                        return [200, packr.pack({ columns, records: statement.all(...requestBody.params) })]
                    } else {
                        statement.run(...requestBody.params)
                        return [200, packr.pack(undefined)]
                    }
                }
            } catch (err) {
                return [400, `${(err as Error).message}\nQuery: ${requestBody.query}\nParams: [${requestBody.params.map((x) => "" + x).join(", ")}]`]
            }
        } catch (err) {
            return [400, (err as Error).message]
        }
    }

    close() {
        this.readonlyConnection.close()

        // Create a noop checkpoint to delete WAL files. https://www.sqlite.org/wal.html#avoiding_excessively_large_wal_files
        this.readWriteConnection.prepare("SELECT * FROM sqlite_master LIMIT 1").all()
        this.readWriteConnection.close()
    }
}

{
    const dbPath = process.env.DB_PATH || "./dev.db"
    if (!fs.existsSync(dbPath)) {
        fs.mkdirSync(path.dirname(dbPath), { recursive: true })
        fs.writeFileSync(dbPath, "")
    }

    const server = new Server(dbPath)
    const state: Record<string, unknown> = {}

    express()
        .use(express.raw())
        .use(cors({ origin: ["http://localhost:5173", "http://127.0.0.1:5173"] }))
        .use("/", express.static("../ui/dist"))
        .post("/query", (req, res) => {
            const [code, responseBody] = server.handle(req.body)
            res.status(code).send(responseBody)
        })
        .post("/import", (req, res) => {
            const { filepath } = unpackr.unpack(req.body as Buffer) as { filepath: string }
            fs.promises.readFile(filepath)
                .then((buf) => { res.send(packr.pack(buf)) })
                .catch((err) => { res.status(400).send(err.message) })
        })
        .post("/export", (req, res) => {
            const { filepath, data } = unpackr.unpack(req.body as Buffer) as { filepath: string, data: Buffer }
            fs.promises.writeFile(filepath, data)
                .then(() => { res.send(packr.pack(null)) })
                .catch((err) => { res.status(400).send(err.message) })
        })
        .post("/setState", (req, res) => {
            const { key, value } = unpackr.unpack(req.body as Buffer) as { key: string, value: unknown }
            state[key] = value
            res.send(packr.pack(null))
        })
        .post("/downloadState", (req, res) => {
            res.send(packr.pack(state))
        })
        .post("/openTerminal", (req, res) => {
            const { text } = unpackr.unpack(req.body as Buffer) as { text: string }
            console.log(text) // TODO
            res.send(packr.pack(null))
        })
        .listen(8080, "127.0.0.1")

    // https://gist.github.com/hyrious/30a878f6e6a057f09db87638567cb11a
    let closed = false
    const close = () => {
        if (closed) { return }
        closed = true
        server.close()
    }
    process.stdin.resume()
    process.on("beforeExit", close)
    process.on("exit", close)
    process.on("SIGTERM", () => { close(); process.exit(1) })
    process.on("SIGINT", () => { close(); process.exit(1) })
}
