import { pack, unpack } from "msgpackr"
import sqlite3 from "sqlite3"

const db = new sqlite3.Database("employees_db-full-1.0.6.db")

db.all("SELECT name FROM sqlite_master WHERE type='table' AND name='departments'", (err: Error | null, rows: Record<string, null | number | string | Buffer>[]) => {
    console.log(rows)
})
