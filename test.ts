import { pack, unpack } from "msgpackr"
import sqlite3 from "sqlite3"

const db = new sqlite3.Database("test.db")

db.all("select * from data_types", (err: Error | null, rows: Record<string, null | number | string | Buffer>[]) => {
    console.log(unpack(pack(rows)))
})
