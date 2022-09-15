import express from "express"
import sqlite3 from "sqlite3"
import cors from "cors"
import { pack, unpack } from "msgpackr"

const app = express()
app.use(cors({ origin: "http://localhost:5173" }))
app.use(express.raw())

const readonlyConnection = new sqlite3.Database("test.db", sqlite3.OPEN_READONLY)
const readWriteConnection = new sqlite3.Database("test.db")

app.post("/", (req, res) => {
    console.log(req.body)
    const query = unpack(req.body) as { query: string, params: (null | number | string | Buffer)[], mode: "r" | "w+" }

    if (typeof query.query !== "string") { res.status(400).send("Invalid search params"); return }
    if (!(Array.isArray(query.params) && query.params.every((p) => p === null || typeof p === "number" || typeof p === "string" || p instanceof Buffer))) { res.status(400).send("Invalid search params"); return }
    if (!["r", "w+"].includes(query.mode)) { res.status(400).send("Invalid search params"); return }

    (query.mode === "w+" ? readWriteConnection : readonlyConnection).all(query.query, query.params, (err, rows) => {
        if (err !== null) { console.error(err); res.status(400).send(err.message) }
        res.send(pack(rows))
    })
})

app.listen(8080, "127.0.0.1")
