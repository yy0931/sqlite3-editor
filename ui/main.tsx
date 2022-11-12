import { render } from "preact"
import * as editor from "./editor"
import deepEqual from "fast-deep-equal"
import { useEffect, useReducer, useRef, Ref, useState } from "preact/hooks"
import SQLite3Client, { Message, TableListItem } from "./sql"
import { Select } from "./components"
import { Table, useTableStore } from "./table"

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
    const [tableName, setTableName] = useState(editor.useEditorStore.getState().tableName!)
    const switchEditorTable = editor.useEditorStore((state) => state.switchTable)

    const [tableList, setTableList] = useState(props.tableList)

    const [viewerStatement, setViewerStatement] = useState<"SELECT" | "PRAGMA">("SELECT")
    const [pragma, setPragma] = useState("analysis_limit")
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
    const scrollerRef = useRef() as Ref<HTMLDivElement>

    props.sql.addErrorMessage = (value) => setErrorMessage((x) => x + value + "\n")

    const queryAndRenderTable = async () => {
        if (tableName === undefined) { return }
        const { wr, type } = tableList.find(({ name }) => name === tableName) ?? {}
        if (wr === undefined || type === undefined) { return }

        // `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case.
        if (viewerStatement === "SELECT") {
            const records = await props.sql.query(`SELECT ${(wr || type !== "table") ? "" : "rowid AS rowid, "}* FROM ${escapeSQLIdentifier(tableName)} ${viewerConstraints} LIMIT ? OFFSET ?`, [pageSize, (page - 1n) * pageSize], "r")
            const newRecordCount = (await props.sql.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(tableName)} ${viewerConstraints}`, [], "r"))[0]!.count
            if (typeof newRecordCount !== "bigint") { throw new Error(newRecordCount + "") }
            setRecordCount(newRecordCount)
            useTableStore.getState().update(
                await props.sql.getTableInfo(tableName),
                records,
                (page - 1n) * pageSize,
                tableName === null ? false : await props.sql.hasTableAutoincrement(tableName),
            )
        } else {
            useTableStore.getState().update(null, (await props.sql.query(`${viewerStatement} ${pragma}`, [], "r")) ?? [], 0n, false)
        }
    }

    useEffect(() => { setPage(page) }, [numRecords, pageSize])

    useEffect(() => {
        queryAndRenderTable().catch(console.error)
    }, [viewerStatement, pragma, tableName, viewerConstraints, tableList, page, pageSize])

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
            const newTableName = opts.selectTable ?? tableName
            if (newTableList.some((table) => table.name === newTableName)) {
                switchEditorTable(newTableName, props.sql)
            } else {
                switchEditorTable(newTableList[0]?.name, props.sql)
            }
            setTableList(newTableList)
        }).catch(console.error)
    }

    return <>
        <ProgressBar />
        <h2 style={{ display: "flex" }}>
            <div style={{ whiteSpace: "pre" }}>
                <Select value={viewerStatement} onChange={(value) => {
                    setViewerStatement(value)
                    if (value === "PRAGMA") {
                        switchEditorTable(undefined, props.sql)
                    } else {
                        switchEditorTable(tableName, props.sql)
                    }
                }} options={{ SELECT: {}, PRAGMA: {} }} className="primary" />
                {viewerStatement === "SELECT" && <> * FROM
                    {" "}
                    {tableName === undefined ? <>No tables</> : <Select value={tableName} onChange={(value) => {
                        setTableName(value)
                        switchEditorTable(value, props.sql)
                    }} options={Object.fromEntries(tableList.map(({ name: tableName }) => [tableName, {}] as const))} className="primary" />}
                    {" "}
                </>}
            </div>
            <div style={{ flex: 1 }}>
                {viewerStatement === "SELECT" && <input value={viewerConstraints} onBlur={(ev) => { setViewerConstraints(ev.currentTarget.value) }} placeholder={"WHERE <column> = <value> ORDER BY <column> ..."} autocomplete="off" style={{ width: "100%" }} />}
                {viewerStatement === "PRAGMA" && <Select value={pragma} onChange={setPragma} options={Object.fromEntries(props.pragmaList.map((k) => [k, {}]))} />}
            </div>
        </h2>
        <div>
            <div ref={scrollerRef} style={{ marginRight: "10px", padding: 0, maxHeight: "50vh", overflowY: "scroll", width: "100%", display: "inline-block" }}>
                {<Table tableName={tableName} sql={props.sql} />}
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
        <editor.Editor tableList={tableList} onWrite={(opts) => { reload(opts) }} sql={props.sql} />
    </>
}

(async () => {
    const sql = new SQLite3Client()
    sql.addErrorMessage = (value) => document.write(value)
    const tableList = await sql.getTableList()
    editor.useEditorStore.setState({ tableName: tableList[0]?.name })
    render(<App
        tableList={tableList}
        pragmaList={(await sql.query("PRAGMA pragma_list", [], "r")).map(({ name }) => name as string)}
        sql={sql} />, document.body)
})().catch(console.error)
