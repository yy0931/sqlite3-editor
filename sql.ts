import { pack, unpack } from "msgpackr"
import { escapeSQLIdentifier } from "./main"

export type TableInfo = { cid: number, dflt_value: number, name: string, notnull: number, type: string, pk: number }[]
export type UniqueConstraints = { primary: boolean, columns: string[] }[]
export type DataTypes = string | number | Uint8Array | null
export type TableListItem = { schema: string, name: string, type: "table" | "view" | "shadow" | "virtual", ncol: number, wr: number, strict: number }

const querying = new Set()
export default class SQLite3Client {
    addErrorMessage: ((value: string) => void) | undefined
    async query(query: string, params: DataTypes[], mode: "r" | "w+"): Promise<Record<string, DataTypes>[]> {
        let res: Response
        const id = {}
        querying.add(id)
        document.body.classList.add("querying")
        try {
            try {
                res = await fetch(`http://localhost:8080/`, { method: "POST", body: pack({ query, params, mode }), headers: { "Content-Type": "application/octet-stream" } })
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
