import { render, } from "preact"
import * as editor from "./editor"
import * as update from "./editor/update"
import * as delete_ from "./editor/delete_"
import * as alter_table from "./editor/alter_table"
import deepEqual from "fast-deep-equal"
import { useState, useEffect, useReducer, useRef, Ref } from "preact/hooks"
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

type TableProps = { tableName: string | null, tableInfo: TableInfo | null, records: Record<string, DataTypes>[], rowStart: number, autoIncrement: boolean }

const Table = ({ records, rowStart, tableInfo, tableName, autoIncrement }: TableProps) => {
    if (tableInfo === null) {
        tableInfo = Object.keys(records[0] ?? {}).map((name) => ({ name, notnull: 0, pk: 0, type: "", cid: 0, dflt_value: 0 }))
    }

    // thead
    return <table className="viewer" style={{ background: "white", width: "max-content" }}>
        <thead>
            <tr>
                <th></th>
                {tableInfo.map(({ name, notnull, pk, type }, i) => <th
                    className={tableName !== null ? "clickable" : ""}
                    onMouseMove={(ev) => {
                        const rect = ev.currentTarget.getBoundingClientRect()
                        if (rect.right - ev.clientX < 10) {
                            ev.currentTarget.classList.add("ew-resize")
                        } else {
                            ev.currentTarget.classList.remove("ew-resize")
                        }
                    }}
                    onMouseDown={(ev) => {
                        const th = ev.currentTarget
                        const rect = th.getBoundingClientRect()
                        if (rect.right - ev.clientX < 10) { // right
                            const mouseMove = (ev: MouseEvent) => {
                                th.style.width = Math.max(50, ev.clientX - rect.left) + "px"
                            }
                            document.body.classList.add("ew-resize")
                            window.addEventListener("mousemove", mouseMove)
                            window.addEventListener("mouseup", () => {
                                window.removeEventListener("mousemove", mouseMove)
                                document.body.classList.remove("ew-resize")
                            }, { once: true })
                        } else if (tableName !== null) { // center
                            alter_table.open(tableName, name)
                        }
                    }}
                    onMouseLeave={(ev) => {
                        ev.currentTarget.classList.remove("ew-resize")
                    }}>
                    <code>
                        {name}
                        <span className="type">{(type ? (" " + type) : "") + (pk ? (autoIncrement ? " PRIMARY KEY AUTOINCREMENT" : " PRIMARY KEY") : "") + (notnull ? " NOT NULL" : "")}</span>
                    </code>
                </th>)}
            </tr>
        </thead>
        <tbody>
            {records.map((record, i) => <tr>
                <td
                    className={tableName !== null ? "clickable" : ""}
                    onClick={(ev) => { if (tableName !== null) { delete_.open(tableName, record, ev.currentTarget.parentElement as HTMLTableRowElement).catch(console.error) } }}>{rowStart + i + 1}</td>
                {tableInfo!.map(({ name }) => {
                    const value = record[name]
                    return <td
                        className={tableName !== null ? "clickable" : ""}
                        onClick={(ev) => { if (tableName !== null) { update.open(tableName, name, record, ev.currentTarget).catch(console.error) } }}>
                        <pre style={{ color: type2color(typeof value) }}>
                            {value instanceof Uint8Array ? `x'${blob2hex(value)}'` :
                                value === null ? "NULL" :
                                    typeof value === "string" ? unsafeEscapeValue(value) :
                                        JSON.stringify(value)}
                        </pre>
                    </td>
                })}
            </tr>)}
        </tbody>
    </table>
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
            if (document.body.classList.contains("querying")) {
                ref.current!.style.left = `${x}px`
                x = (Date.now() - t) % (window.innerWidth + width) - width
            }
            requestAnimationFrame(loop)
        }
        loop()
        return () => { canceled = true }
    }, [])
    return <div className="progressbar" ref={ref} style={{ display: "inline-block", userSelect: "none", pointerEvents: "none", position: "absolute", zIndex: 100, width: width + "px", height: "5px", top: 0, background: "var(--button-primary-background)" }}></div>
}

const App = (props: { tableList: TableListItem[], pragmaList: string[], sql: SQLite3Client }) => {
    const [tableList, setTableList] = useState(props.tableList)

    const [viewerStatement, setViewerStatement] = useState<"SELECT" | "PRAGMA">("SELECT")
    const [pragma, setPragma] = useState("analysis_limit")
    const [viewerTableName, setViewerTableName] = useState(tableList[0]?.name) // undefined if there are no tables
    const [viewerConstraints, setViewerConstraints] = useState("")
    const [errorMessage, setErrorMessage] = useState("")
    const [pageSize, setPageSize] = useReducer<number, number>((_, value) => Math.max(1, value), 1000)
    const [numRecords, setRecordCount] = useState(0)
    const pageMax = Math.ceil(numRecords / pageSize)
    const [_, rerender] = useState({})
    const [page, setPage] = useReducer<number, number>((_, value) => {
        const clippedValue = Math.max(1, Math.min(pageMax, value))
        if (value !== clippedValue) { rerender({}) }  // Update the input box when value !== clippedValue === oldValue
        return clippedValue
    }, 0)
    const [tableProps, setTableProps] = useState<TableProps | null>(null)

    props.sql.addErrorMessage = (value) => setErrorMessage((x) => x + value + "\n")

    const queryAndRenderTable = async () => {
        if (viewerTableName === undefined) { return }
        const { wr, type } = tableList.find(({ name }) => name === viewerTableName) ?? {}
        if (wr === undefined || type === undefined) { return }

        // `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case.
        if (viewerStatement === "SELECT") {
            const records = await props.sql.query(`SELECT ${(wr || type !== "table") ? "" : "rowid AS rowid, "}* FROM ${escapeSQLIdentifier(viewerTableName)} ${viewerConstraints} LIMIT ? OFFSET ?`, [pageSize, (page - 1) * pageSize], "r")
            const newRecordCount = (await props.sql.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(viewerTableName)} ${viewerConstraints}`, [], "r"))[0]!.count
            if (typeof newRecordCount !== "number") { throw new Error(newRecordCount + "") }
            setRecordCount(newRecordCount)
            setTableProps({
                tableName: viewerTableName,
                autoIncrement: viewerTableName === null ? false : await props.sql.hasTableAutoincrement(viewerTableName),
                records,
                rowStart: (page - 1) * pageSize,
                tableInfo: await props.sql.getTableInfo(viewerTableName),
            })
        } else {
            setTableProps({
                tableName: null,
                autoIncrement: false,
                records: await props.sql.query(`${viewerStatement} ${pragma}`, [], "r"),
                rowStart: 0,
                tableInfo: null,
            })
        }
    }

    useEffect(() => { setPage(page) }, [numRecords, pageSize])

    useEffect(() => {
        queryAndRenderTable().catch(console.error)
    }, [viewerStatement, pragma, viewerTableName, viewerConstraints, tableList, page, pageSize])

    return <>
        <ProgressBar />
        {viewerTableName !== undefined && <h2>
            <Select value={viewerStatement} onChange={setViewerStatement} options={{ SELECT: {}, PRAGMA: {} }} className="primary" />
            {viewerStatement === "SELECT" && <> * FROM
                {" "}
                <Select value={viewerTableName} onChange={setViewerTableName} options={Object.fromEntries(tableList.map(({ name: tableName }) => [tableName, {}] as const))} className="primary" />
                {" "}
                <input value={viewerConstraints} onBlur={(ev) => { setViewerConstraints(ev.currentTarget.value) }} placeholder={"WHERE <column> = <value> ORDER BY <column> ..."} autocomplete="off" style={{ width: "1000px" }} /></>}
            {viewerStatement === "PRAGMA" && <Select value={pragma} onChange={setPragma} options={Object.fromEntries(props.pragmaList.map((k) => [k, {}]))} />}
        </h2>}
        <div>
            <div style={{ marginLeft: "10px", marginRight: "10px", padding: 0, maxHeight: "50vh", overflowY: "scroll", width: "100%", display: "inline-block" }}>
                {tableProps && <Table {...tableProps} />}
            </div>
        </div>
        <div style={{ marginBottom: "30px", paddingTop: "3px" }} className="primary">
            <span><span style={{ cursor: "pointer", paddingLeft: "8px", paddingRight: "8px", userSelect: "none" }} onClick={() => setPage(page - 1)}>‹</span><input value={page} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onChange={(ev) => setPage(+ev.currentTarget.value)} /> / {pageMax} <span style={{ cursor: "pointer", paddingLeft: "4px", paddingRight: "8px", userSelect: "none" }} onClick={() => setPage(page + 1)}>›</span></span>
            <span style={{ marginLeft: "40px" }}><input value={pageSize} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onBlur={(ev) => setPageSize(+ev.currentTarget.value)} /> records</span>
        </div>
        {errorMessage && <p style={{ background: "rgb(14, 72, 117)", color: "white", padding: "10px" }}>
            <pre>{errorMessage}</pre>
            <input type="button" value="Close" className="primary" style={{ marginTop: "10px" }} onClick={() => setErrorMessage("")} />
        </p>}
        <editor.Editor tableName={viewerStatement === "SELECT" ? viewerTableName : undefined} tableList={tableList} onWrite={(opts) => {
            const skipTableRefresh = opts.refreshTableList || opts.selectTable !== undefined
            if (!skipTableRefresh) {
                queryAndRenderTable().catch(console.error)
            }
            props.sql.getTableList().then((newTableList) => {
                if (deepEqual(newTableList, tableList)) {
                    if (skipTableRefresh) {
                        queryAndRenderTable().catch(console.error)
                    }
                    return
                }
                const newViewerTableName = opts.selectTable ?? viewerTableName
                if (newTableList.some((table) => table.name === newViewerTableName)) {
                    setViewerTableName(newViewerTableName)
                } else {
                    setViewerTableName(newTableList[0]?.name)
                }
                setTableList(newTableList)
            }).catch(console.error)
        }} sql={props.sql} />
    </>
}

(async () => {
    const sql = new SQLite3Client()
    sql.addErrorMessage = (value) => document.write(value)
    render(<App
        tableList={await sql.getTableList()}
        pragmaList={(await sql.query("PRAGMA pragma_list", [], "r")).map(({ name }) => name as string)}
        sql={sql} />, document.body)
})().catch(console.error)
