import express from "express"
import sqlite3 from "better-sqlite3"
import { Packr, Unpackr } from "msgpackr"
import fs from "fs"

const packr = new Packr({ useRecords: false, preserveNumericTypes: true })
const unpackr = new Unpackr({ largeBigIntToFloat: false, int64AsNumber: false, mapsAsObjects: true, useRecords: true, preserveNumericTypes: true })

const readonlyConnection = sqlite3("../samples/employees_db-full-1.0.6.db", { readonly: true })
const readWriteConnection = sqlite3("../samples/employees_db-full-1.0.6.db")

readonlyConnection.defaultSafeIntegers()
readWriteConnection.defaultSafeIntegers()

const find_widget_regexp = (text: string, pattern: string, wholeWord: 0n | 1n, caseSensitive: 0n | 1n) => new RegExp(wholeWord ? `\\b(?:${pattern})\\b` : pattern, caseSensitive ? "" : "i").test(text) ? 1n : 0n
readonlyConnection.function("find_widget_regexp", { deterministic: true, varargs: false, safeIntegers: true }, find_widget_regexp)
readWriteConnection.function("find_widget_regexp", { deterministic: true, varargs: false, safeIntegers: true }, find_widget_regexp)

const state: Record<string, unknown> = {}

express()
    .use(express.raw())
    .use("/", express.static("../../ui/dist"))
    .post("/query", (req, res) => {
        try {
            const query = unpackr.unpack(req.body as Buffer) as { query: string, params: (null | bigint | number | string | Buffer)[], mode: "r" | "w+" }

            if (typeof query.query !== "string") { throw new Error(`Invalid arguments`) }
            if (!(Array.isArray(query.params) && query.params.every((p) => p === null || typeof p === "number" || typeof p === "bigint" || typeof p === "string" || p instanceof Buffer))) { throw new Error(`Invalid arguments`) }
            if (!["r", "w+"].includes(query.mode)) { throw new Error(`Invalid arguments`) }

            try {
                const statement = (query.mode === "w+" ? readWriteConnection : readonlyConnection).prepare(query.query)
                if (statement.reader) {
                    res.send(packr.pack(statement.all(...query.params)))
                } else {
                    statement.run(...query.params)
                    res.send(packr.pack(undefined))
                }
            } catch (err) {
                throw new Error(`${(err as Error).message}\nQuery: ${query.query}\nParams: [${query.params.map((x) => "" + x).join(", ")}]`)
            }
        } catch (err) {
            res.status(400).send((err as Error).message)
        }
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
    .listen(8080, "127.0.0.1")
