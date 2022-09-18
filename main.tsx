import { render, } from "preact"
import { pack, unpack } from "msgpackr"
import * as editor from "./editor"
import * as update from "./editor/update"
import * as insert from "./editor/insert"
import deepEqual from "deep-equal"
import { useState, useEffect, useMemo } from "preact/hooks"
import { Select } from "./editor/components"

export type DataTypes = string | number | Uint8Array | null

type TableListItem = { schema: string, name: string, type: string, ncol: number, wr: number, strict: number }

export const sql = async (query: string, params: DataTypes[], mode: "r" | "w+"): Promise<Record<string, DataTypes>[]> => {
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

const renderTable = async (tableName: string, { wr: withoutRowId }: TableListItem, tableInfo: TableInfo, constraints: string) => {
    const autoIncrement = await hasTableAutoincrement(tableName)

    const all = await sql(`SELECT ${withoutRowId ? "" : /* `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case. */"rowid AS rowid, "}* FROM ${escapeSQLIdentifier(tableName)} ` + constraints, [], "r") as Record<string, DataTypes>[]

    document.querySelector<HTMLTableElement>("#table")!.innerHTML = ""

    // thead
    {
        const thead = document.createElement("thead")
        const tr = document.createElement("tr")
        thead.append(tr)
        for (const { name, notnull, pk, type } of tableInfo) {
            const th = document.createElement("th")
            const code = document.createElement("code")
            code.innerText = name
            const typeText = document.createElement("span")
            typeText.classList.add("type")
            typeText.innerText = (type ? (" " + type) : "") + (pk ? (autoIncrement ? " PRIMARY KEY AUTOINCREMENT" : " PRIMARY KEY") : "") + (notnull ? " NOT NULL" : "")
            code.append(typeText)
            th.append(code)
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
                    update.open(tableName, name, record, td).catch(console.error)
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
    for (const column of await getTableInfo(tableName)) {
        if (column.pk) {
            uniqueConstraints.push({ primary: true, columns: [column.name] })
        }
    }
    for (const index of await sql(`PRAGMA index_list(${escapeSQLIdentifier(tableName)})`, [], "r") as { seq: number, name: string, unique: 0 | 1, origin: "c" | "u" | "pk", partial: 0 | 1 }[]) {
        if (index.partial) { continue }
        if (!index.unique) { continue }
        const indexInfo = await sql(`PRAGMA index_info(${escapeSQLIdentifier(index.name)})`, [], "r") as { seqno: number, cid: number, name: string }[]
        uniqueConstraints.push({ primary: index.origin === "pk", columns: indexInfo.map(({ name }) => name) })
    }
    return uniqueConstraints
}

/** https://stackoverflow.com/questions/20979239/how-to-tell-if-a-sqlite-column-is-autoincrement */
export const hasTableAutoincrement = async (tableName: string) =>
    !!((await sql(`SELECT COUNT(*) AS count FROM sqlite_sequence WHERE name = ?`, [tableName], "r"))[0]?.count)

const App = (props: { tableList: TableListItem[] }) => {
    const [tableList, setTableList] = useState(props.tableList)

    const [viewerStatement, setViewerStatement] = useState<"SELECT" | "PRAGMA table_list">("SELECT")
    const [viewerTableName, setViewerTableName] = useState(tableList[0]?.name) // undefined if there are no tables
    const [viewerConstraints, setViewerConstraints] = useState("")

    const renderTable_ = async () => {
        if (viewerTableName === undefined) { return }
        await insert.open(viewerTableName) // TODO: Change to insert or create table only if the current statement is UPDATE
        await renderTable(viewerTableName, tableList.find(({ name }) => name === viewerTableName)!, await getTableInfo(viewerTableName), viewerConstraints)
    }

    useEffect(() => {
        renderTable_().catch(console.error)
    }, [viewerStatement, viewerTableName, viewerConstraints, tableList])

    return <>
        {viewerTableName !== undefined && <h2>
            <pre><Select value={viewerStatement} onChange={setViewerStatement} options={{ SELECT: {}, "PRAGMA table_list": {} }} style={{ color: "white", background: "var(--accent-color)" }} /> * FROM
                {" "}
                <Select value={viewerTableName} onChange={setViewerTableName} options={Object.fromEntries(tableList.map(({ name: tableName }) => [tableName, {}] as const))} style={{ color: "white", background: "var(--accent-color)" }} />
                {" "}
                <input value={viewerConstraints} onChange={(ev) => { setViewerConstraints(ev.currentTarget.value) }} placeholder={"WHERE <column> = <value> ORDER BY <column> ..."} autocomplete="off" style={{ width: "1000px" }} />
            </pre>
        </h2>}
        {useMemo(() => <div class="scroll">
            <table id="table"></table>
        </div>, [])}
        <editor.Editor tableName={viewerTableName} onWrite={() => {
            renderTable_().catch(console.error)
            getTableList().then((newTableList) => {
                if (deepEqual(newTableList, tableList, { strict: true })) { return }
                setTableList(newTableList)
            }).catch(console.error)
        }} />
    </>
}

(async () => {
    render(<App tableList={await getTableList()} />, document.body)
})().catch(console.error)
