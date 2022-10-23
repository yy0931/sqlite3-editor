import { render } from "preact"
import * as editor from "./editor"
import * as update from "./editor/update"
import * as delete_ from "./editor/delete_"
import * as alter_table from "./editor/alter_table"
import deepEqual from "fast-deep-equal"
import { useState, useEffect, useReducer, useRef, Ref } from "preact/hooks"
import { Select } from "./editor/components"
import SQLite3Client, { DataTypes, Message, TableInfo, TableListItem } from "./sql"

/** https://stackoverflow.com/a/6701665/10710682, https://stackoverflow.com/a/51574648/10710682 */
export const escapeSQLIdentifier = (ident: string) => {
    if (ident.includes("\x00")) { throw new Error("Invalid identifier") }
    return ident.includes('"') || /[^A-Za-z0-9_\$]/.test(ident) ? `"${ident.replaceAll('"', '""')}"` : ident
}

/** For presentation only */
export const unsafeEscapeValue = (x: unknown): string =>
    typeof x === "string" ? `'${x.replaceAll("'", "''")}'` : x + ""

export const blob2hex = (blob: Uint8Array, maxLength?: number) =>
    Array.from(blob.slice(0, maxLength), (x) => x.toString(16).padStart(2, "0")).join("") + (maxLength !== undefined && blob.length > maxLength ? "..." : "")

export const type2color = (type: string) => {
    if (type === "number" || type === "bigint") {
        return "green"
    } else if (type === "string") {
        return "rgb(138, 4, 4)"
    } else {
        return "rgb(4, 63, 138)"
    }
}

type TableProps = { tableName: string | null, tableInfo: TableInfo | null, records: Record<string, DataTypes>[], rowStart: bigint, autoIncrement: boolean }

const Table = ({ records, rowStart, tableInfo, tableName, autoIncrement }: TableProps) => {
    if (tableInfo === null) {
        tableInfo = Object.keys(records[0] ?? {}).map((name) => ({ name, notnull: 0n, pk: 0n, type: "", cid: 0n, dflt_value: 0n }))
    }

    const columnWidths = useRef<(number | null)[]>(Object.keys(records[0] ?? {}).map(() => null))
    const tableRef = useRef() as Ref<HTMLTableElement>

    // thead
    return <table ref={tableRef} className="viewer" style={{ background: "white", width: "max-content" }}>
        <thead>
            <tr>
                <th></th>
                {tableInfo.map(({ name, notnull, pk, type }, i) => <th
                    style={{ width: columnWidths.current[i] }}
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
                                columnWidths.current[i] = Math.max(50, ev.clientX - rect.left)
                                th.style.width = columnWidths.current[i] + "px"
                                for (const td of tableRef.current?.querySelectorAll<HTMLElement>(`td:nth-child(${i + 2})`) ?? []) {
                                    td.style.maxWidth = columnWidths.current[i] + "px"
                                }
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
            {records.length === 0 && <tr>
                <td className="no-hover" style={{ display: "inline-block", height: "1.2em", cursor: "default" }}></td>
            </tr>}
            {records.map((record, i) => <tr>
                <td
                    className={tableName !== null ? "clickable" : ""}
                    onClick={(ev) => { if (tableName !== null) { delete_.open(tableName, record, ev.currentTarget.parentElement as HTMLTableRowElement).catch(console.error) } }}>{rowStart + BigInt(i) + 1n}</td>
                {tableInfo!.map(({ name }, i) => {
                    const value = record[name] as DataTypes
                    return <td
                        style={{ maxWidth: columnWidths.current[i] }}
                        className={tableName !== null ? "clickable" : ""}
                        onClick={(ev) => { if (tableName !== null) { update.open(tableName, name, record, ev.currentTarget).catch(console.error) } }}>
                        <pre style={{ color: type2color(typeof value) }}>
                            {value instanceof Uint8Array ? `x'${blob2hex(value, 8)}'` :
                                value === null ? "NULL" :
                                    typeof value === "string" ? unsafeEscapeValue(value) :
                                        typeof value === "number" ? (/^[+\-]?\d+$/.test("" + value) ? "" + value + ".0" : "" + value) :
                                            "" + value}
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

const BigintMath = {
    max: (...args: bigint[]) => args.reduce((prev, curr) => curr > prev ? curr : prev),
    min: (...args: bigint[]) => args.reduce((prev, curr) => curr < prev ? curr : prev),
}

const App = (props: { tableList: TableListItem[], pragmaList: string[], sql: SQLite3Client }) => {
    const [tableList, setTableList] = useState(props.tableList)

    const [viewerStatement, setViewerStatement] = useState<"SELECT" | "PRAGMA">("SELECT")
    const [pragma, setPragma] = useState("analysis_limit")
    const [viewerTableName, setViewerTableName] = useState(tableList[0]?.name) // undefined if there are no tables
    const [viewerConstraints, setViewerConstraints] = useState("")
    const [errorMessage, setErrorMessage] = useState("")
    const [pageSize, setPageSize] = useReducer<bigint, bigint>((_, value) => BigintMath.max(1n, value), 1000n)
    const [numRecords, setRecordCount] = useState(0n)
    const pageMax = BigInt(Math.ceil(Number(numRecords) / Number(pageSize)))
    const [_, rerender] = useState({})
    const [page, setPage] = useReducer<bigint, bigint>((_, value) => {
        const clippedValue = BigintMath.max(1n, BigintMath.min(pageMax, value))
        if (value !== clippedValue) { rerender({}) }  // Update the input box when value !== clippedValue === oldValue
        return clippedValue
    }, 0n)
    const [tableProps, setTableProps] = useState<TableProps | null>(null)
    const scrollerRef = useRef() as Ref<HTMLDivElement>

    props.sql.addErrorMessage = (value) => setErrorMessage((x) => x + value + "\n")

    const queryAndRenderTable = async () => {
        if (viewerTableName === undefined) { return }
        const { wr, type } = tableList.find(({ name }) => name === viewerTableName) ?? {}
        if (wr === undefined || type === undefined) { return }

        // `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case.
        if (viewerStatement === "SELECT") {
            const records = await props.sql.query(`SELECT ${(wr || type !== "table") ? "" : "rowid AS rowid, "}* FROM ${escapeSQLIdentifier(viewerTableName)} ${viewerConstraints} LIMIT ? OFFSET ?`, [pageSize, (page - 1n) * pageSize], "r")
            const newRecordCount = BigInt((await props.sql.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(viewerTableName)} ${viewerConstraints}`, [], "r"))[0]!.count as number | bigint)  // TODO: Remove BigInt() after https://github.com/kriszyp/msgpackr/issues/78 is resolved
            if (typeof newRecordCount !== "bigint") { throw new Error(newRecordCount + "") }
            setRecordCount(newRecordCount)
            setTableProps({
                tableName: viewerTableName,
                autoIncrement: viewerTableName === null ? false : await props.sql.hasTableAutoincrement(viewerTableName),
                records,
                rowStart: (page - 1n) * pageSize,
                tableInfo: await props.sql.getTableInfo(viewerTableName),
            })
        } else {
            setTableProps({
                tableName: null,
                autoIncrement: false,
                records: (await props.sql.query(`${viewerStatement} ${pragma}`, [], "r")) ?? [],
                rowStart: 0n,
                tableInfo: null,
            })
        }
    }

    useEffect(() => { setPage(page) }, [numRecords, pageSize])

    useEffect(() => {
        queryAndRenderTable().catch(console.error)
    }, [viewerStatement, pragma, viewerTableName, viewerConstraints, tableList, page, pageSize])

    const [reloadRequired, setReloadRequired] = useState(false)

    useEffect(() => {
        const handler = ({ data }: Message) => {
            if (data.type === "sqlite3-editor-server" && data.requestId === undefined) {
                setReloadRequired(true)
            }
        }
        window.addEventListener("message", handler)
        return () => { window.removeEventListener("message", handler) }
    }, [])

    {
        const reloadRequiredRef = useRef(false)
        useEffect(() => { reloadRequiredRef.current = reloadRequired }, [reloadRequired])
        useEffect(() => {
            const timer = setInterval(() => {
                if (reloadRequiredRef.current) {
                    reload({ refreshTableList: true })
                }
            }, 1000)
            return () => { clearInterval(timer) }
        }, [reloadRequired])
    }

    const reload = (opts: editor.OnWriteOptions) => {
        setReloadRequired(false)
        const skipTableRefresh = opts.refreshTableList || opts.selectTable !== undefined
        if (!skipTableRefresh) {
            queryAndRenderTable()
                .then(() => {
                    if (opts.scrollToBottom) {
                        setTimeout(() => {  // TODO: remove setTimeout
                            setPage(pageMax)
                            scrollerRef.current?.scrollBy({ behavior: "smooth", top: scrollerRef.current!.scrollHeight - scrollerRef.current!.offsetHeight })
                        }, 80)
                    }
                })
                .catch(console.error)
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
    }

    return <>
        <ProgressBar />
        {viewerTableName !== undefined && <h2 style={{ display: "flex" }}>
            <div style={{ whiteSpace: "pre" }}>
                <Select value={viewerStatement} onChange={setViewerStatement} options={{ SELECT: {}, PRAGMA: {} }} className="primary" />
                {viewerStatement === "SELECT" && <> * FROM
                    {" "}
                    <Select value={viewerTableName} onChange={setViewerTableName} options={Object.fromEntries(tableList.map(({ name: tableName }) => [tableName, {}] as const))} className="primary" />
                    {" "}
                </>}
            </div>
            <div style={{ flex: 1 }}>
                {viewerStatement === "SELECT" && <input value={viewerConstraints} onBlur={(ev) => { setViewerConstraints(ev.currentTarget.value) }} placeholder={"WHERE <column> = <value> ORDER BY <column> ..."} autocomplete="off" style={{ width: "100%" }} />}
                {viewerStatement === "PRAGMA" && <Select value={pragma} onChange={setPragma} options={Object.fromEntries(props.pragmaList.map((k) => [k, {}]))} />}
            </div>
        </h2>}
        <div>
            <div ref={scrollerRef} style={{ marginRight: "10px", padding: 0, maxHeight: "50vh", overflowY: "scroll", width: "100%", display: "inline-block" }}>
                {tableProps && <Table {...tableProps} />}
            </div>
        </div>
        <div style={{ marginBottom: "30px", paddingTop: "3px" }} className="primary">
            <span><span style={{ cursor: "pointer", paddingLeft: "8px", paddingRight: "8px", userSelect: "none" }} onClick={() => setPage(page - 1n)}>‹</span><input value={"" + page} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onChange={(ev) => setPage(BigInt(ev.currentTarget.value))} /> / {pageMax} <span style={{ cursor: "pointer", paddingLeft: "4px", paddingRight: "8px", userSelect: "none" }} onClick={() => setPage(page + 1n)}>›</span></span>
            <span style={{ marginLeft: "40px" }}><input value={"" + pageSize} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onBlur={(ev) => setPageSize(BigInt(ev.currentTarget.value))} /> records</span>
        </div>
        {errorMessage && <p style={{ background: "rgb(14, 72, 117)", color: "white", padding: "10px" }}>
            <pre>{errorMessage}</pre>
            <input type="button" value="Close" className="primary" style={{ marginTop: "10px" }} onClick={() => setErrorMessage("")} />
        </p>}
        <editor.Editor tableName={viewerStatement === "SELECT" ? viewerTableName : undefined} tableList={tableList} onWrite={(opts) => { reload(opts) }} sql={props.sql} />
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
