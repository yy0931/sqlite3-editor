import { pack, unpack } from "msgpackr"
import { escapeSQLIdentifier } from "./main"

export type TableInfo = { cid: number, dflt_value: number, name: string, notnull: number, type: string, pk: number }[]
export type UniqueConstraints = { primary: boolean, columns: string[] }[]
export type DataTypes = string | number | Uint8Array | null
export type TableListItem = { schema: string, name: string, type: "table" | "view" | "shadow" | "virtual", ncol: number, wr: number, strict: number }

type VSCodeAPI = { postMessage(data: unknown): void }

declare global {
    interface Window {
        acquireVsCodeApi?: () => VSCodeAPI
    }
}

const vscode = window.acquireVsCodeApi?.()

export type Message = { data: { requestId: undefined } | { requestId: number } & ({ err: string } | { body: Uint8Array }) }

const querying = new Set()
export default class SQLite3Client {
    addErrorMessage: ((value: string) => void) | undefined

    async #postVSCode(body: unknown) {
        if (vscode === undefined) { return }
        const id = {}
        querying.add(id)
        document.body.classList.add("querying")
        return new Promise<Record<string, DataTypes>[]>((resolve, reject) => {
            const requestId = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER)
            vscode.postMessage({ requestId, body: pack(body) })
            const callback = ({ data }: Message) => {
                if (data.requestId !== requestId) { return }
                window.removeEventListener("message", callback)
                if ("err" in data) {
                    this.addErrorMessage?.(data.err)
                    reject(new Error(data.err))
                    return
                }
                resolve(unpack(data.body))
            }
            window.addEventListener("message", callback)
        }).finally(() => {
            querying.delete(id)
            if (querying.size === 0) {
                document.body.classList.remove("querying")
            }
        })
    }

    async #postHTTP(body: unknown) {
        const id = {}
        querying.add(id)
        document.body.classList.add("querying")
        try {
            let res: Response
            try {
                res = await fetch(`/query`, { method: "POST", body: pack(body), headers: { "Content-Type": "application/octet-stream" } })
            } catch (err: any) {
                this.addErrorMessage?.("message" in err ? err.message : "" + err)
                throw err
            }
            if (!res.ok) {
                const message = await res.text()
                this.addErrorMessage?.(message)
                throw new Error(message)
            }
            return unpack(new Uint8Array(await res.arrayBuffer()))
        } finally {
            querying.delete(id)
            if (querying.size === 0) {
                document.body.classList.remove("querying")
            }
        }
    }

    query = (query: string, params: DataTypes[], mode: "r" | "w+"): Promise<Record<string, DataTypes>[]> =>
        vscode ? this.#postVSCode({ query, params, mode }) : this.#postHTTP({ query, params, mode })

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
        for (const index of await this.query(`PRAGMA index_list(${escapeSQLIdentifier(tableName)})`, [], "r") as { seq: number, name: string, unique: 0 | 1, origin: "c" | "u" | "pk", partial: 0 | 1 }[]) {
            if (index.partial) { continue }
            if (!index.unique) { continue }
            const indexInfo = await this.query(`PRAGMA index_info(${escapeSQLIdentifier(index.name)})`, [], "r") as { seqno: number, cid: number, name: string }[]
            uniqueConstraints.push({ primary: index.origin === "pk", columns: indexInfo.map(({ name }) => name) })
        }
        return uniqueConstraints
    }

    getTableInfo = async (tableName: string) =>
        this.query(`PRAGMA table_info(${escapeSQLIdentifier(tableName)})`, [], "r") as Promise<TableInfo>

    getTableList = async () => this.query("PRAGMA table_list", [], "r") as Promise<TableListItem[]>
}
