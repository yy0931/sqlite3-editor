import express from "express"
import sqlite3 from "sqlite3"
import { pack, unpack } from "msgpackr"

const readonlyConnection = new sqlite3.Database("samples/employees_db-full-1.0.6.db", sqlite3.OPEN_READONLY)
const readWriteConnection = new sqlite3.Database("samples/employees_db-full-1.0.6.db")

const query = (req: { body: Buffer }) => new Promise<Buffer>((resolve, reject) => {
    const query = unpack(req.body) as { query: string, params: (null | number | string | Buffer)[], mode: "r" | "w+" }

    if (typeof query.query !== "string") { reject(new Error(`Invalid search params: ${JSON.stringify(query)}`)); return }
    if (!(Array.isArray(query.params) && query.params.every((p) => p === null || typeof p === "number" || typeof p === "string" || p instanceof Buffer))) { reject(new Error(`Invalid search params: ${JSON.stringify(query)}`)); return }
    if (!["r", "w+"].includes(query.mode)) { reject(new Error(`Invalid search params: ${JSON.stringify(query)}`)); return }

    (query.mode === "w+" ? readWriteConnection : readonlyConnection).all(query.query, query.params, (err, rows) => {
        if (err !== null) { console.error(err); reject(new Error(`${err.message}\nQuery: ${query.query}\nParams: ${JSON.stringify(query.params)}`)); return }
        resolve(pack(rows))
    })
})

express()
    .use(express.raw())
    .use("/", express.static("../../ui/dist"))
    .post("/query", (req, res) => {
        query(req)
            .then((data) => { res.send(data) })
            .catch((err: Error) => { res.status(400).send(err.message) })
    })
    .listen(8080, "127.0.0.1", () => { console.log(`http://localhost:8080`) })
