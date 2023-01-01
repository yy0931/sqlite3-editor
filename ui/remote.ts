import { Packr, Unpackr } from "msgpackr"
import { useMainStore } from "./main"
import { escapeSQLIdentifier } from "./table"

export type TableInfo = { cid: bigint, dflt_value: bigint | string, name: string, notnull: bigint, type: string, pk: bigint }[]
export type UniqueConstraints = { primary: boolean, columns: string[] }[]
export type SQLite3Value = string | number | bigint | Uint8Array | null
export type TableListItem = { schema: string, name: string, type: "table" | "view" | "shadow" | "virtual", ncol: bigint, wr: bigint, strict: bigint }

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

/** Send the data to the server with fetch(), or to the extension host with vscode.postMessage(). */
export const post = async (url: string, body: unknown) => {
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
                    useMainStore.getState().addErrorMessage(data.err)
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
            } catch (err: any) {
                useMainStore.getState().addErrorMessage("message" in err ? err.message : "" + err)
                throw err
            }
            if (!res.ok) {
                const message = await res.text()
                useMainStore.getState().addErrorMessage(message)
                throw new Error(message)
            }
            return unpackr.unpack(new Uint8Array(await res.arrayBuffer()))
        } finally {
            queue.splice(queue.indexOf(id)!, 1)
        }
    }
}

let state: Record<string, unknown> = {}
export const setState = async <T>(key: string, value: T) => {
    state[key] = value
    await post("/setState", { key, value })
}
export const getState = <T>(key: string): T | undefined => {
    return state[key] as T | undefined
}
/** Called only once, before any getState() or setState() calls. */
export const downloadState = (): Promise<void> => post("/downloadState", {}).then((value) => { state = value })

type QueryResult<T extends string> = Promise<T extends `SELECT ${string}` | `PRAGMA pragma_list` ? Record<string, SQLite3Value>[] : Record<string, SQLite3Value>[] | undefined>

export const query = <T extends string>(query: T, params: SQLite3Value[], mode: "r" | "w+"): QueryResult<T> =>
    post(`/query`, { query, params, mode })

/** Imports a BLOB from a file. */
export const import_ = (filepath: string) =>
    post(`/import`, { filepath }) as Promise<Uint8Array>

/** Exports a BLOB to a file. */
export const export_ = (filepath: string, data: Uint8Array) =>
    post(`/export`, { filepath, data }) as Promise<void>

/** https://stackoverflow.com/a/1604121/10710682 */
export const existsTable = async (tableName: string) =>
    (await query(`SELECT name FROM sqlite_master WHERE type='table' AND name=?`, [tableName], "r")).length > 0

/** https://stackoverflow.com/questions/20979239/how-to-tell-if-a-sqlite-column-is-autoincrement */
export const hasTableAutoincrementColumn = async (tableName: string) =>
    await existsTable("sqlite_sequence") &&  // sqlite_sequence doesn't exist when the database is empty.
    !!((await query(`SELECT COUNT(*) AS count FROM sqlite_sequence WHERE name = ?`, [tableName], "r"))[0]?.count)

export type IndexInfo = { seqno: bigint, cid: bigint, name: string }[]

export const getIndexInfo = (indexName: string) =>
    query(`PRAGMA index_info(${escapeSQLIdentifier(indexName)})`, [], "r") as Promise<IndexInfo>

export type IndexList = { seq: bigint, name: string, unique: 0n | 1n, origin: "c" | "u" | "pk", partial: 0n | 1n }[]

export const getIndexList = (tableName: string) =>
    query(`PRAGMA index_list(${escapeSQLIdentifier(tableName)})`, [], "r") as Promise<IndexList>

export const getTableInfo = (tableName: string) =>
    query(`PRAGMA table_info(${escapeSQLIdentifier(tableName)})`, [], "r") as Promise<TableInfo>

export const getTableList = async () => {
    return (await query("PRAGMA table_list", [], "r") as TableListItem[])
        .filter(({ name }) => !name.startsWith("sqlite_"))  // https://www.sqlite.org/fileformat2.html#intschema
}

export const getTableSchema = async (tableName: string) =>
    (await query(`SELECT sql FROM sqlite_schema WHERE name = ?`, [tableName], "r"))[0]?.sql as string | undefined

export const getIndexSchema = async (indexName: string) =>
    (await query(`SELECT sql FROM sqlite_schema WHERE type = ? AND name = ?`, ["index", indexName], "r"))[0]?.sql as string | null | undefined
