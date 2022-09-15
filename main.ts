import { pack, unpack } from "msgpackr"

type DataTypes = string | number | Uint8Array | null

type TableListItem = { schema: string, name: string, type: string, ncol: number, wr: number, strict: number }

const sql = async (query: string, params: DataTypes[], mode: "r" | "w+") => {
    const res = await fetch(`http://localhost:8080/`, { method: "POST", body: pack({ query, params, mode }), headers: { "Content-Type": "application/octet-stream" } })
    if (!res.ok) {
        throw new Error(await res.text())
    }
    return unpack(new Uint8Array(await res.arrayBuffer()))
}

/** https://stackoverflow.com/a/6701665/10710682, https://stackoverflow.com/a/51574648/10710682 */
const escapeSQLIdentifier = (ident: string) => {
    if (ident.includes("\x00")) { throw new Error("Invalid identifier") }
    return ident.includes('"') || /[^A-Za-z0-9_\$]/.test(ident) ? `"${ident.replaceAll('"', '""')}"` : ident
}

/** For presentation only */
const unsafeEscapeValue = (x: unknown): string =>
    typeof x === "string" ? `'${x.replaceAll("'", "''")}'` : x + ""

let buildQueryFromEditorContent: ((editorContent: DataTypes) => { query: string, params: DataTypes[] }) | null = null

const type2color = (type: string) => {
    if (type === "number") {
        return "green"
    } else if (type === "string") {
        return "rgb(138, 4, 4)"
    } else {
        return "inherit"
    }
}

document.querySelector<HTMLSelectElement>("#updateEditorType")!.addEventListener("change", () => {
    document.querySelector<HTMLTextAreaElement>("#updateEditor")!.style.color = type2color(document.querySelector<HTMLSelectElement>("#updateEditorType")!.value)
})

const closeUpdateEditor = () => {
    document.querySelector<HTMLTextAreaElement>("#updateEditorContainer")!.hidden = true
    document.querySelector<HTMLElement>("#editorTitle")!.innerHTML = ""
    buildQueryFromEditorContent = null
    document.querySelectorAll(".editing").forEach((el) => el.classList.remove("editing"))
    if (document.querySelector<HTMLSelectElement>("#editorStatement")!.value === "UPDATE") {
        document.querySelector<HTMLSelectElement>("#editorStatement")!.value = "INSERT"
        document.querySelector<HTMLSelectElement>("#editorStatement")!.dispatchEvent(new Event("change"))
    }
}

type TableInfo = { cid: number, dflt_value: number, name: string, notnull: number, type: string, pk: number }[]

const renderTable = async (tableName: string, { wr: withoutRowId }: TableListItem, uniqueConstraints: { primary: boolean, columns: string[] }[], tableInfo: TableInfo) => {
    closeUpdateEditor()

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
                const blob2hex = (blob: Uint8Array, maxLength = 8) =>
                    Array.from(blob.slice(0, maxLength), (x) => x.toString(16).padStart(2, "0")).join("") + (blob.length > maxLength ? "..." : "")
                pre.innerText = value instanceof Uint8Array ? `x'${blob2hex(value)}'` : JSON.stringify(value)
                td.append(pre)
                tr.append(td)
                td.addEventListener("click", () => {
                    closeUpdateEditor()
                    document.querySelector<HTMLSelectElement>("#editorStatement")!.value = "UPDATE"
                    document.querySelector<HTMLElement>("#editorTitle")!.innerText = `${escapeSQLIdentifier(tableName)} SET ${escapeSQLIdentifier(name)} = ? `
                    const updateConstraintSelect = document.createElement("select")
                    for (const columns of uniqueConstraints.sort((a, b) => +b.primary - +a.primary).map(({ columns }) => columns).concat(withoutRowId ? [] : [["rowid"]])) {
                        const option = document.createElement("option")
                        if (columns.some((column) => record[column] === null)) { continue }  // columns under unique constraints can have multiple NULLs
                        option.innerText = columns.map((column) => `WHERE ${column} = ${unsafeEscapeValue(record[column])}`).join(" ")
                        option.value = JSON.stringify(columns)
                        updateConstraintSelect.append(option)
                    }
                    if (updateConstraintSelect.options.length === 0) {  // no options
                        throw new Error()
                    }
                    document.querySelector<HTMLElement>("#editorTitle")!.append(updateConstraintSelect)
                    document.querySelector<HTMLTextAreaElement>("#updateEditor")!.value = value instanceof Uint8Array ? blob2hex(value) : (value + "")
                    document.querySelector<HTMLTextAreaElement>("#updateEditorContainer")!.hidden = false
                    document.querySelector<HTMLSelectElement>("#updateEditorType")!.value =
                        value === null ? "null" :
                            value instanceof Uint8Array ? "blob" :
                                typeof value === "number" ? "number" :
                                    "string"
                    document.querySelector<HTMLSelectElement>("#updateEditorType")!.dispatchEvent(new Event("change"))
                    td.classList.add("editing")
                    buildQueryFromEditorContent = (editorContent) => {
                        const columns = JSON.parse(updateConstraintSelect.value) as string[]
                        return { query: `UPDATE ${escapeSQLIdentifier(tableName)} SET ${escapeSQLIdentifier(name)} = ? ` + columns.map((column) => `WHERE ${column} = ?`).join(" "), params: [editorContent, ...columns.map((column) => record[column] as DataTypes)] }
                    }
                })
            }
        }
        document.querySelector<HTMLTableElement>("#table")!.append(tbody)
    }
}

const main = async () => {
    const listUniqueConstraints = async (tableName: string) => {
        const uniqueConstraints: { primary: boolean, columns: string[] }[] = []
        for (const index of await sql(`PRAGMA index_list(${escapeSQLIdentifier(tableName)})`, [], "r") as { seq: number, name: string, unique: 0 | 1, origin: "c" | "u" | "pk", partial: 0 | 1 }[]) {
            if (index.partial) { continue }
            if (!index.unique) { continue }
            const indexInfo = await sql(`PRAGMA index_info(${index.name})`, [], "r") as { seqno: number, cid: number, name: string }[]
            uniqueConstraints.push({ primary: index.origin === "pk", columns: indexInfo.map(({ name }) => name) })
        }
        return uniqueConstraints
    }
    const getTableInfo = async (tableName: string) =>
        sql(`PRAGMA table_info(${escapeSQLIdentifier(tableName)})`, [], "r") as Promise<TableInfo>

    const tableList = await sql("PRAGMA table_list", [], "r") as TableListItem[]
    for (const { name: tableName } of tableList) {
        const option = document.createElement("option")
        option.innerText = option.value = tableName
        document.querySelector("#tableSelect")!.append(option)
    }

    const openTable = async () => {
        const tableName = document.querySelector<HTMLSelectElement>("#tableSelect")!.value
        await renderTable(tableName, tableList.find(({ name }) => name === tableName)!, await listUniqueConstraints(tableName), await getTableInfo(tableName))
    }

    if (document.querySelector<HTMLSelectElement>("#tableSelect")!.value) { await openTable() }
    document.querySelector<HTMLSelectElement>("#tableSelect")!.addEventListener("change", () => { openTable() })
    document.querySelector<HTMLInputElement>("#constraints")!.addEventListener("change", () => { openTable() })

    window.addEventListener("click", (ev) => {
        if (ev.target instanceof HTMLElement && !ev.target.matches("div.scroll")) { return }
        closeUpdateEditor()
    })

    document.querySelector<HTMLTextAreaElement>("#updateEditor")!.addEventListener("change", () => {
        if (buildQueryFromEditorContent === null) { return }
        const value = document.querySelector<HTMLTextAreaElement>("#updateEditor")!.value
        const type = document.querySelector<HTMLSelectElement>("#updateEditorType")!.value
        const { query, params } = buildQueryFromEditorContent(
            type === "null" ? null :
                type === "number" ? +value :
                    type === "blob" ? Uint8Array.from(value.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) ?? /* TODO: Show an error message*/[]) :
                        value)
        sql(query, params, "w+").then(openTable)
        closeUpdateEditor()
    })

    document.querySelector<HTMLSelectElement>("#editorStatement")!.addEventListener("change", async () => {
        try {
            closeUpdateEditor()
            const statement = document.querySelector<HTMLSelectElement>("#editorStatement")!.value
            if (statement === "INSERT") {
                const tableName = document.querySelector<HTMLSelectElement>("#tableSelect")!.value
                const columns = (await getTableInfo(tableName)).map(({ name }) => name)
                document.querySelector<HTMLElement>("#editorTitle")!.innerHTML = `INTO ${escapeSQLIdentifier(tableName)} (${columns.map(escapeSQLIdentifier).join(", ")}) VALUES (${columns.map(() => "?").join(", ")})`
            } else if (statement === "CREATE TABLE") {
                // TODO:
            }
        } catch (err) {
            console.error(err)
        }
    })
    document.querySelector<HTMLSelectElement>("#editorStatement")!.dispatchEvent(new Event("change"))
}

main().catch(console.error)
