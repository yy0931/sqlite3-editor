import { render, } from "preact"
import * as editor from "./editor"
import * as update from "./editor/update"
import * as insert from "./editor/insert"
import deepEqual from "deep-equal"
import { useState, useEffect, useMemo, useReducer, useRef, Ref } from "preact/hooks"
import { Select } from "./editor/components"
import SQLite3Client, { DataTypes, TableInfo, TableListItem } from "./sql"

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

const renderTable = async (tableName: string, tableInfo: TableInfo, records: Record<string, DataTypes>[], sql: SQLite3Client) => {
    document.body.classList.add("rendering")
    try {
        const autoIncrement = await sql.hasTableAutoincrement(tableName)

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
            for (const record of records) {
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
    } finally {
        document.body.classList.remove("rendering")
    }
}

const ProgressBar = () => {
    const ref = useRef() as Ref<HTMLDivElement>
    let x = 0
    const width = 200
    const t = Date.now()
    useEffect(() => {
        if (ref.current === null) { return }
        let canceled = false
        const loop = () => {
            if (canceled) { return }
            ref.current!.style.left = `${x}px`
            x = (Date.now() - t) % (window.innerWidth + width) - width
            requestAnimationFrame(loop)
        }
        loop()
        return () => { canceled = true }
    }, [])
    return <div className="progressbar" ref={ref} style={{ display: "inline-block", userSelect: "none", pointerEvents: "none", position: "absolute", zIndex: 100, width: width + "px", height: "5px", top: 0, background: "var(--accent-color)" }}></div>
}

const App = (props: { tableList: TableListItem[], sql: SQLite3Client }) => {
    const [tableList, setTableList] = useState(props.tableList)

    const [viewerStatement, setViewerStatement] = useState<"SELECT" | "PRAGMA table_list">("SELECT")
    const [viewerTableName, setViewerTableName] = useState(tableList[0]?.name) // undefined if there are no tables
    const [viewerConstraints, setViewerConstraints] = useState("")
    const [errorMessage, setErrorMessage] = useState("")
    const [pageSize, setPageSize] = useReducer<number, number>((_, value) => Math.max(1, value), 1000)
    const [numRecords, setRecordCount] = useState(0)
    const pageMax = Math.ceil(numRecords / pageSize)
    const [page, setPage] = useReducer<number, number>((_, value) => Math.max(1, Math.min(pageMax, value)), 0)

    props.sql.addErrorMessage = (value) => setErrorMessage((x) => x + value + "\n")

    useEffect(() => {
        if (viewerTableName === undefined) { return }
        insert.open(viewerTableName)  // TODO: Change to insert or create table only if the current statement is UPDATE
    }, [viewerTableName])

    const queryAndRenderTable = async () => {
        if (viewerTableName === undefined) { return }
        // `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case.
        const records = await props.sql.query(`SELECT ${tableList.find(({ name }) => name === viewerTableName)!.wr ? "" : "rowid AS rowid, "}* FROM ${escapeSQLIdentifier(viewerTableName)} ${viewerConstraints} LIMIT ? OFFSET ?`, [pageSize, (page - 1) * pageSize], "r")
        const newRecordCount = (await props.sql.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(viewerTableName)} ${viewerConstraints}`, [], "r"))[0]!.count
        await renderTable(viewerTableName, await props.sql.getTableInfo(viewerTableName), records, props.sql)
        if (typeof newRecordCount !== "number") { throw new Error(newRecordCount + "") }
        setRecordCount(newRecordCount)
    }

    useEffect(() => { setPage(page) }, [numRecords, pageSize])

    useEffect(() => {
        queryAndRenderTable().catch(console.error)
    }, [viewerStatement, viewerTableName, viewerConstraints, tableList, page, pageSize])

    return <>
        <ProgressBar />
        {viewerTableName !== undefined && <h2>
            <pre><Select value={viewerStatement} onChange={setViewerStatement} options={{ SELECT: {}, "PRAGMA table_list": {} }} className="primary" /> * FROM
                {" "}
                <Select value={viewerTableName} onChange={setViewerTableName} options={Object.fromEntries(tableList.map(({ name: tableName }) => [tableName, {}] as const))} className="primary" />
                {" "}
                <input value={viewerConstraints} onChange={(ev) => { setViewerConstraints(ev.currentTarget.value) }} placeholder={"WHERE <column> = <value> ORDER BY <column> ..."} autocomplete="off" style={{ width: "1000px" }} /><br />
            </pre>
        </h2>}
        {useMemo(() => <div class="scroll">
            <table id="table"></table>
        </div>, [])}
        <div style={{ marginBottom: "30px", paddingTop: "3px", paddingBottom: "3px" }} className="primary">
            <span><span style={{ cursor: "pointer", paddingLeft: "8px", paddingRight: "8px", userSelect: "none" }} onClick={() => setPage(page - 1)}>‹</span><input value={page} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onChange={(ev) => setPage(+ev.currentTarget.value)} /> / {pageMax} <span style={{ cursor: "pointer", paddingLeft: "4px", paddingRight: "8px", userSelect: "none" }} onClick={() => setPage(page + 1)}>›</span></span>
            <span style={{ marginLeft: "40px" }}><input value={pageSize} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onBlur={(ev) => setPageSize(+ev.currentTarget.value)} /> records</span>
        </div>
        {errorMessage && <p style={{ background: "rgb(14, 72, 117)", color: "white", padding: "10px" }}>
            <pre style={{ whiteSpace: "pre-wrap" }}>{errorMessage}</pre>
            <input type="button" value="Close" className="primary" style={{ marginTop: "10px" }} onClick={() => setErrorMessage("")} />
        </p>}
        <editor.Editor tableName={viewerTableName} onWrite={() => {
            queryAndRenderTable().catch(console.error)
            props.sql.getTableList().then((newTableList) => {
                if (deepEqual(newTableList, tableList, { strict: true })) { return }
                setTableList(newTableList)
            }).catch(console.error)
        }} sql={props.sql} />
    </>
}

(async () => {
    const sql = new SQLite3Client()
    sql.addErrorMessage = (value) => document.write(value)
    render(<App tableList={await sql.getTableList()} sql={sql} />, document.body)
})().catch(console.error)
