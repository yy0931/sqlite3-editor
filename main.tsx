import { render, } from "preact"
import { pack, unpack } from "msgpackr"
import * as editor from "./editor"
import * as update from "./editor/update"
import * as insert from "./editor/insert"

export type DataTypes = string | number | Uint8Array | null

type TableListItem = { schema: string, name: string, type: string, ncol: number, wr: number, strict: number }

export const sql = async (query: string, params: DataTypes[], mode: "r" | "w+") => {
    const res = await fetch(`http://localhost:8080/`, { method: "POST", body: pack({ query, params, mode }), headers: { "Content-Type": "application/octet-stream" } })
    if (!res.ok) {
        throw new Error(await res.text())
    }
    return unpack(new Uint8Array(await res.arrayBuffer()))
}

/** https://stackoverflow.com/a/6701665/10710682, https://stackoverflow.com/a/51574648/10710682 */
export const escapeSQLIdentifier = (ident: string) => {
    if (ident.includes("\x00")) { throw new Error("Invalid identifier") }
    return ident.includes('"') || /[^A-Za-z0-9_\$]/.test(ident) ? `"${ident.replaceAll('"', '""')}"` : ident
}

/** For presentation only */
export const unsafeEscapeValue = (x: unknown): string =>
    typeof x === "string" ? `'${x.replaceAll("'", "''")}'` : x + ""

export const blob2hex = (blob: Uint8Array, maxLength = 8) =>
    Array.from(blob.slice(0, maxLength), (x) => x.toString(16).padStart(2, "0")).join("") + (blob.length > maxLength ? "..." : "")

export const type2color = (type: string) => {
    if (type === "number") {
        return "green"
    } else if (type === "string") {
        return "rgb(138, 4, 4)"
    } else {
        return "rgb(4, 63, 138)"
    }
}

export type TableInfo = { cid: number, dflt_value: number, name: string, notnull: number, type: string, pk: number }[]

export type UniqueConstraints = { primary: boolean, columns: string[] }[]

const renderTable = async (tableName: string, { wr: withoutRowId }: TableListItem, uniqueConstraints: UniqueConstraints, tableInfo: TableInfo) => {
    await insert.open()

    const all = await sql(`SELECT ${withoutRowId ? "" : "rowid, "}* FROM ${escapeSQLIdentifier(tableName)} ` + document.querySelector<HTMLInputElement>("#constraints")!.value, [], "r") as Record<string, DataTypes>[]

    document.querySelector<HTMLTableElement>("#table")!.innerHTML = ""

    // thead
    {
        const thead = document.createElement("thead")
        const tr = document.createElement("tr")
        thead.append(tr)
        for (const { name, notnull, pk, type } of tableInfo) {
            const th = document.createElement("th")
            const pre = document.createElement("pre")
            pre.innerText = name + (notnull ? " NOT NULL" : "") + (type ? (" " + type) : "")
            th.append(pre)
            tr.append(th)
        }
        document.querySelector<HTMLTableElement>("#table")!.append(thead)
    }

    // tbody
    {
        const tbody = document.createElement("tbody")
        for (const record of all) {
            const tr = document.createElement("tr")
            tbody.append(tr)
            for (const { name } of tableInfo) {
                const value = record[name]
                if (value === undefined) { throw new Error() }
                const td = document.createElement("td")
                const pre = document.createElement("pre")
                pre.style.color = type2color(typeof value)
                pre.innerText = value instanceof Uint8Array ? `x'${blob2hex(value)}'` : JSON.stringify(value)
                td.append(pre)
                tr.append(td)
                td.addEventListener("click", () => {
                    update.open(name, record, td).catch(console.error)
                })
            }
        }
        document.querySelector<HTMLTableElement>("#table")!.append(tbody)
    }
}

export const getTableInfo = async (tableName: string) =>
    sql(`PRAGMA table_info(${escapeSQLIdentifier(tableName)})`, [], "r") as Promise<TableInfo>

export const getTableList = async () => sql("PRAGMA table_list", [], "r") as Promise<TableListItem[]>

export const listUniqueConstraints = async (tableName: string) => {
    const uniqueConstraints: { primary: boolean, columns: string[] }[] = []
    for (const index of await sql(`PRAGMA index_list(${escapeSQLIdentifier(tableName)})`, [], "r") as { seq: number, name: string, unique: 0 | 1, origin: "c" | "u" | "pk", partial: 0 | 1 }[]) {
        if (index.partial) { continue }
        if (!index.unique) { continue }
        const indexInfo = await sql(`PRAGMA index_info(${index.name})`, [], "r") as { seqno: number, cid: number, name: string }[]
        uniqueConstraints.push({ primary: index.origin === "pk", columns: indexInfo.map(({ name }) => name) })
    }
    return uniqueConstraints
}

const main = async () => {
    const tableList = await getTableList()
    document.querySelector("#tableSelect")!.innerHTML = ""
    for (const { name: tableName } of tableList) {
        const option = document.createElement("option")
        option.innerText = option.value = tableName
        document.querySelector("#tableSelect")!.append(option)
    }
    render(<editor.Editor refreshTable={() => openTable()} />, document.querySelector("#editor")!)

    const openTable = async () => {
        const tableName = document.querySelector<HTMLSelectElement>("#tableSelect")!.value
        await renderTable(tableName, tableList.find(({ name }) => name === tableName)!, await listUniqueConstraints(tableName), await getTableInfo(tableName))
    }

    if (document.querySelector<HTMLSelectElement>("#tableSelect")!.value) { await openTable() }
    document.querySelector<HTMLSelectElement>("#tableSelect")!.addEventListener("change", () => { openTable() })
    document.querySelector<HTMLInputElement>("#constraints")!.addEventListener("change", () => { openTable() })
}

main().catch(console.error)
