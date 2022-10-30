import { Packr, Unpackr } from "msgpackr"
import { escapeSQLIdentifier } from "./main"

export type TableInfo = { cid: bigint, dflt_value: bigint, name: string, notnull: bigint, type: string, pk: bigint }[]
export type UniqueConstraints = { primary: boolean, columns: string[] }[]
export type DataTypes = string | number | bigint | Uint8Array | null
export type TableListItem = { schema: string, name: string, type: "table" | "view" | "shadow" | "virtual", ncol: bigint, wr: bigint, strict: bigint }

type VSCodeAPI = { postMessage(data: unknown): void }

declare global {
    interface Window {
        acquireVsCodeApi?: () => VSCodeAPI
    }
}

const vscode = window.acquireVsCodeApi?.()

export type Message = { data: { /* preact debugger also uses message events */ type: "sqlite3-editor-server" } & ({ requestId: undefined } | { requestId: number } & ({ err: string } | { body: Uint8Array })) }

const packr = new Packr({ useRecords: false, preserveNumericTypes: true })
const unpackr = new Unpackr({ largeBigIntToFloat: false, int64AsNumber: false, mapsAsObjects: true, useRecords: true, preserveNumericTypes: true })

const querying = new Set()
export default class SQLite3Client {
    addErrorMessage: ((value: string) => void) | undefined

    async #post(url: string, body: unknown) {
        const id = {}
        querying.add(id)
        document.body.classList.add("querying")
        if (vscode !== undefined) {
            return new Promise<unknown>((resolve, reject) => {
                const requestId = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER)
                vscode.postMessage({ requestId, path: url, body: packr.pack(body) })
                const callback = ({ data }: Message) => {
                    if (data.type !== "sqlite3-editor-server" || data.requestId !== requestId) { return }
                    window.removeEventListener("message", callback)
                    if ("err" in data) {
                        this.addErrorMessage?.(data.err)
                        reject(new Error(data.err))
                        return
                    }
                    resolve(unpackr.unpack(data.body))
                }
                window.addEventListener("message", callback)
            }).finally(() => {
                querying.delete(id)
                if (querying.size === 0) {
                    document.body.classList.remove("querying")
                }
            })
        } else {
            try {
                let res: Response
                try {
                    res = await fetch(url, { method: "POST", body: packr.pack(body), headers: { "Content-Type": "application/octet-stream" } })
                } catch (err: any) {
                    this.addErrorMessage?.("message" in err ? err.message : "" + err)
                    throw err
                }
                if (!res.ok) {
                    const message = await res.text()
                    this.addErrorMessage?.(message)
                    throw new Error(message)
                }
                return unpackr.unpack(new Uint8Array(await res.arrayBuffer()))
            } finally {
                querying.delete(id)
                if (querying.size === 0) {
                    document.body.classList.remove("querying")
                }
            }
        }
    }

    query = <T extends string>(query: T, params: DataTypes[], mode: "r" | "w+"): Promise<
        T extends `SELECT ${string}` | `PRAGMA pragma_list` ?
        Record<string, DataTypes>[] :
        Record<string, DataTypes>[] | undefined> =>
        this.#post(`/query`, { query, params, mode })

    import = (filepath: string) =>
        this.#post(`/import`, { filepath }) as Promise<Uint8Array>

    export = (filepath: string, data: Uint8Array) =>
        this.#post(`/export`, { filepath, data }) as Promise<void>

    /** https://stackoverflow.com/a/1604121/10710682 */
    hasTable = async (tableName: string) =>
        (await this.query(`SELECT name FROM sqlite_master WHERE type='table' AND name=?`, [tableName], "r")).length > 0

    /** https://stackoverflow.com/questions/20979239/how-to-tell-if-a-sqlite-column-is-autoincrement */
    hasTableAutoincrement = async (tableName: string) =>
        await this.hasTable("sqlite_sequence") &&
        !!((await this.query(`SELECT COUNT(*) AS count FROM sqlite_sequence WHERE name = ?`, [tableName], "r"))[0]?.count)

    listUniqueConstraints = async (tableName: string) => {
        const uniqueConstraints: { primary: boolean, columns: string[] }[] = []
        for (const column of await this.getTableInfo(tableName)) {
            if (column.pk) {
                uniqueConstraints.push({ primary: true, columns: [column.name] })
            }
        }
        for (const index of await this.query(`PRAGMA index_list(${escapeSQLIdentifier(tableName)})`, [], "r") as { seq: bigint, name: string, unique: 0n | 1n, origin: "c" | "u" | "pk", partial: 0n | 1n }[]) {
            if (index.partial) { continue }
            if (!index.unique) { continue }
            const indexInfo = await this.query(`PRAGMA index_info(${escapeSQLIdentifier(index.name)})`, [], "r") as { seqno: bigint, cid: bigint, name: string }[]
            uniqueConstraints.push({ primary: index.origin === "pk", columns: indexInfo.map(({ name }) => name) })
        }
        return uniqueConstraints
    }

    getTableInfo = async (tableName: string) =>
        this.query(`PRAGMA table_info(${escapeSQLIdentifier(tableName)})`, [], "r") as Promise<TableInfo>

    getTableList = async () => this.query("PRAGMA table_list", [], "r") as Promise<TableListItem[]>
}
