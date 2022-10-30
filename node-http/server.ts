import express from "express"
import sqlite3 from "better-sqlite3"
import { pack, unpack } from "msgpackr"
import fs from "fs"

const readonlyConnection = sqlite3("../samples/employees_db-full-1.0.6.db", { readonly: true })
const readWriteConnection = sqlite3("../samples/employees_db-full-1.0.6.db")

readonlyConnection.defaultSafeIntegers()
readWriteConnection.defaultSafeIntegers()

express()
    .use(express.raw())
    .use("/", express.static("../../ui/dist"))
    .post("/query", (req, res) => {
        try {
            const query = unpack(req.body as Buffer) as { query: string, params: (null | bigint | number | string | Buffer)[], mode: "r" | "w+" }

            if (typeof query.query !== "string") { throw new Error(`Invalid arguments`) }
            if (!(Array.isArray(query.params) && query.params.every((p) => p === null || typeof p === "number" || typeof p === "bigint" || typeof p === "string" || p instanceof Buffer))) { throw new Error(`Invalid arguments`) }
            if (!["r", "w+"].includes(query.mode)) { throw new Error(`Invalid arguments`) }

            try {
                const statement = (query.mode === "w+" ? readWriteConnection : readonlyConnection).prepare(query.query)
                if (statement.reader) {
                    res.send(pack(statement.all(...query.params)))
                } else {
                    statement.run(...query.params)
                    res.send(pack(undefined))
                }
            } catch (err) {
                throw new Error(`${(err as Error).message}\nQuery: ${query.query}\nParams: [${query.params.map((x) => "" + x).join(", ")}]`)
            }
        } catch (err) {
            res.status(400).send((err as Error).message)
        }
    })
    .post("/import", (req, res) => {
        const { filepath } = unpack(req.body as Buffer) as { filepath: string }
        fs.promises.readFile(filepath)
            .then((buf) => { res.send(pack(buf)) })
            .catch((err) => { res.status(400).send(err.message) })
    })
    .post("/export", (req, res) => {
        const { filepath, data } = unpack(req.body as Buffer) as { filepath: string, data: Buffer }
        fs.promises.writeFile(filepath, data)
            .then(() => { res.send(pack(null)) })
            .catch((err) => { res.status(400).send(err.message) })
    })
    .listen(8080, "127.0.0.1", () => { console.log(`http://localhost:8080`) })
