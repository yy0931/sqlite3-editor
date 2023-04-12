import { Packr, Unpackr } from "msgpackr"
import { escapeSQLIdentifier, useTableStore } from "./table"

export type TableInfo = { cid: bigint, dflt_value: bigint | string | null, name: string, notnull: bigint, type: string, pk: bigint }[]
export type UniqueConstraints = { primary: boolean, columns: string[] }[]
export type SQLite3Value = string | number | bigint | Uint8Array | null
export type TableListItem = { name: string, type: "table" | "view" | "shadow" | "virtual", wr: bigint, strict: bigint }

export type Message = { data: { /* preact debugger also uses message events */ type: "sqlite3-editor-server" } & ({ requestId: undefined } | { requestId: number } & ({ err: string } | { body: Uint8Array })) }

const packr = new Packr({ useRecords: false, preserveNumericTypes: true })
const unpackr = new Unpackr({ largeBigIntToFloat: false, int64AsNumber: false, mapsAsObjects: true, useRecords: true, preserveNumericTypes: true })

const vscode = window.acquireVsCodeApi?.()

const queue: { start: number }[] = []

const loop = () => {
    if (queue.length > 0 && Date.now() - queue[0]!.start > 1000) {
        document.body.classList.add("querying")
    } else {
        document.body.classList.remove("querying")
    }
    requestAnimationFrame(loop)
}
loop()

type PostOptions = { withoutLogging?: boolean }

/** Send the data to the server with fetch(), or to the extension host with vscode.postMessage(). */
export const post = async (url: string, body: unknown, opts: PostOptions = {}) => {
    const id = { start: Date.now() }
    queue.push(id)
    if (vscode !== undefined) {
        return new Promise<unknown>((resolve, reject) => {
            const requestId = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER)
            vscode.postMessage({ requestId, path: url, body: packr.pack(body) })
            const callback = ({ data }: Message) => {
                if (data.type !== "sqlite3-editor-server" || data.requestId !== requestId) { return }
                window.removeEventListener("message", callback)
                if ("err" in data) {
                    if (!opts.withoutLogging) { useTableStore.getState().addErrorMessage(data.err) }
                    reject(new Error(data.err))
                    return
                }
                resolve(unpackr.unpack(data.body))
            }
            window.addEventListener("message", callback)
        }).finally(() => {
            queue.splice(queue.indexOf(id)!, 1)
        })
    } else {
        try {
            let res: Response
            try {
                res = await fetch("http://localhost:8080" + url, { method: "POST", body: packr.pack(body), headers: { "Content-Type": "application/octet-stream" } })
            } catch (err) {
                if (!opts.withoutLogging) { useTableStore.getState().addErrorMessage(typeof err === "object" && err !== null && "message" in err ? err.message + "" : err + "") }
                throw err
            }
            if (!res.ok) {
                const message = await res.text()
                if (!opts.withoutLogging) { useTableStore.getState().addErrorMessage(message) }
                throw new Error(message)
            }
            return unpackr.unpack(new Uint8Array(await res.arrayBuffer()))
        } finally {
            queue.splice(queue.indexOf(id)!, 1)
        }
    }
}

/** A local copy of the persisted data in the server. */
let state: Record<string, unknown> = {}

/** Retrieves and stores the persisted data in the server. */
export const downloadState = (opts: PostOptions = {}): Promise<void> => post("/downloadState", {}, opts).then((value) => { state = value })

/** Stores the value with the given key to the server. */
export const setState = async <T>(key: string, value: T, opts: PostOptions = {} = {}) => {
    state[key] = value
    await post("/setState", { key, value }, opts)
}

/** Returns the value associated with the given key from local copy of the persisted data in the server. `downloadState()` have be called beforehand. */
export const getState = <T>(key: string): T | undefined => {
    return state[key] as T | undefined
}

type QueryResult<T extends string> = Promise<{
    columns: string[]
    records:
    T extends `SELECT ${string}` ? Record<string, SQLite3Value>[] :
    T extends `PRAGMA pragma_list` ? { name: string[] } :
    (Record<string, SQLite3Value>[] | undefined)
}>

/** Queries the database, and commits if `mode` is "w+". */
export const query = <T extends string>(query: T, params: readonly SQLite3Value[], mode: "r" | "w+", opts: PostOptions = {}): QueryResult<T> =>
    post(`/query`, { query, params, mode }, opts)

/** Imports a BLOB from a file. */
export const import_ = (filepath: string, opts: PostOptions = {}) =>
    post(`/import`, { filepath }, opts) as Promise<Uint8Array>

/** Exports a BLOB to a file. */
export const export_ = (filepath: string, data: Uint8Array, opts: PostOptions = {}) =>
    post(`/export`, { filepath, data }, opts) as Promise<void>

/** https://stackoverflow.com/a/1604121/10710682 */
export const existsTable = async (tableName: string, opts: PostOptions = {}) =>
    (await query(`SELECT name FROM sqlite_master WHERE type='table' AND name=?`, [tableName], "r", opts)).records.length > 0

/** https://stackoverflow.com/questions/20979239/how-to-tell-if-a-sqlite-column-is-autoincrement */
export const hasTableAutoincrementColumn = async (tableName: string, opts: PostOptions = {}) =>
    await existsTable("sqlite_sequence", opts) &&  // sqlite_sequence doesn't exist when the database is empty.
    !!((await query(`SELECT COUNT(*) AS count FROM sqlite_sequence WHERE name = ?`, [tableName], "r", opts)).records[0]?.count)

export type IndexInfo = { seqno: bigint, cid: bigint, name: string }[]

/** Queries `PRAGMA index_info(indexName)`. */
export const getIndexInfo = async (indexName: string, opts: PostOptions = {}) =>
    (await query(`PRAGMA index_info(${escapeSQLIdentifier(indexName)})`, [], "r", opts)).records as IndexInfo

export type IndexList = { seq: bigint, name: string, unique: 0n | 1n, origin: "c" | "u" | "pk", partial: 0n | 1n }[]

/** Queries `PRAGMA index_list(tableName)`. */
export const getIndexList = async (tableName: string, opts: PostOptions = {}) =>
    (await query(`PRAGMA index_list(${escapeSQLIdentifier(tableName)})`, [], "r", opts)).records as IndexList

/** Queries `PRAGMA table_info(tableName)`. */
export const getTableInfo = async (tableName: string, opts: PostOptions = {}) =>
    (await query(`PRAGMA table_info(${escapeSQLIdentifier(tableName)})`, [], "r", opts)).records as TableInfo

export let isSQLiteOlderThan3_37 = false

/** Lists tables in the database, excluding internal tables. */
export const getTableList = async (opts: PostOptions = {}) => {
    const tableList = await query("PRAGMA table_list", [], "r", opts)  // Requires SQLite >= 3.37.0
    if (!tableList) {  // if using SQLite < 3.37.0
        isSQLiteOlderThan3_37 = true
        const tableList: TableListItem[] = []
        for (const { name, type, sql } of (await query(`SELECT name, type, sql FROM sqlite_master WHERE NOT (name LIKE "sqlite\\_%" ESCAPE "\\") AND type IN ("table", "view", "shadow", "virtual")`, [], "r", opts)).records as { name: string, type: "table" | "view" | "shadow" | "virtual", sql: string }[]) {
            tableList.push({
                name,
                type,
                wr: /WITHOUT\s*ROWID[^()]*$/.test(sql) ? 1n : 0n,
                strict: 0n, // The STRICT option is introduced in SQLite 3.37.
            })
        }
        return tableList
    }
    return (tableList.records as TableListItem[])
        .filter(({ name }) => !name.startsWith("sqlite_"))  // https://www.sqlite.org/fileformat2.html#intschema
}

/** Retrieves the table schema. */
export const getTableSchema = async (tableName: string, opts: PostOptions = {}) =>
    // sqlite_master is renamed to sqlite_master in 3.33.0, but sqlite_master is kept as an alias https://sqlite.org/forum/forumpost/889b616b87a7020a
    (await query(`SELECT sql FROM sqlite_master WHERE name = ?`, [tableName], "r", opts)).records[0]?.sql as string | undefined

/** Retrieves the index schema. */
export const getIndexSchema = async (indexName: string, opts: PostOptions = {}) =>
    (await query(`SELECT sql FROM sqlite_master WHERE type = ? AND name = ?`, ["index", indexName], "r", opts)).records[0]?.sql as string | null | undefined
