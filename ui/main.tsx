import { render } from "preact"
import * as editor from "./editor"
import deepEqual from "fast-deep-equal"
import { useEffect, useRef, Ref } from "preact/hooks"
import SQLite3Client, { Message, TableListItem } from "./sql"
import { Select } from "./components"
import { Table, useTableStore } from "./table"
import zustand from "zustand"

/** https://stackoverflow.com/a/6701665/10710682, https://stackoverflow.com/a/51574648/10710682 */
export const escapeSQLIdentifier = (ident: string) => {
    if (ident.includes("\x00")) { throw new Error("Invalid identifier") }
    return ident.includes('"') || /[^A-Za-z0-9_\$]/.test(ident) ? `"${ident.replaceAll('"', '""')}"` : ident
}

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

export const useMainStore = zustand<{
    sql: SQLite3Client
    reloadRequired: boolean
    paging: {
        page: bigint
        numRecords: bigint
        pageSize: bigint
    }
    errorMessage: string
    viewerStatement: "SELECT" | "PRAGMA"
    pragma: string
    viewerConstraints: string
    tableName: string | undefined
    tableList: TableListItem[]
    pragmaList: string[]
    scrollerRef: { current: HTMLDivElement | null }
    _rerender: {},
    requireReloading: () => void
    rerender: () => void,
    getPageMax: () => bigint
    reload: (opts: editor.OnWriteOptions) => Promise<void>
    setPaging: (opts: { page?: bigint, numRecords?: bigint, pageSize?: bigint }) => void
    addErrorMessage: (value: string) => void
    queryAndRenderTable: () => Promise<void>
}>()((set, get) => {
    return {
        sql: new SQLite3Client(),
        reloadRequired: false,
        getPageMax: () => BigInt(Math.ceil(Number(get().paging.numRecords) / Number(get().paging.pageSize))),
        tableName: undefined,
        paging: {
            page: 1n,
            numRecords: 0n,
            pageSize: 1000n,
        },
        errorMessage: "",
        viewerStatement: "SELECT",
        pragma: "analysis_limit",
        viewerConstraints: "",
        tableList: [],
        pragmaList: [],
        scrollerRef: { current: null },
        _rerender: {},
        requireReloading: () => { set({ reloadRequired: true }) },
        setPaging: (opts) => {
            const newValue = { ...get().paging, ...opts }
            newValue.pageSize = BigintMath.max(1n, newValue.pageSize)

            const pageMax = BigInt(Math.ceil(Number(newValue.numRecords) / Number(newValue.pageSize)))
            const clippedPage = BigintMath.max(1n, BigintMath.min(pageMax, newValue.page))
            if (newValue.page !== clippedPage) { get().rerender() }  // Update the input box when value !== clippedPage === oldValue
            set({ paging: { page: clippedPage, numRecords: newValue.numRecords, pageSize: newValue.pageSize } })
        },
        rerender: () => set({ _rerender: {} }),
        addErrorMessage: (value: string) => set((s) => ({ errorMessage: s.errorMessage + value + "\n" })),
        reload: async (opts: editor.OnWriteOptions) => {
            set({ reloadRequired: false })
            const state = get()
            const skipTableRefresh = opts.refreshTableList || opts.selectTable !== undefined
            const handles = new Array<Promise<void>>()
            if (!skipTableRefresh) {
                handles.push(state.queryAndRenderTable()
                    .then(() => {
                        if (opts.scrollToBottom) {
                            setTimeout(() => {  // TODO: remove setTimeout
                                get().setPaging({ page: state.getPageMax() })
                                state.scrollerRef.current?.scrollBy({ behavior: "smooth", top: get().scrollerRef.current!.scrollHeight - get().scrollerRef.current!.offsetHeight })
                            }, 80)
                        }
                    })
                    .catch(console.error))
            }
            handles.push(state.sql.getTableList().then((newTableList) => {
                if (deepEqual(newTableList, state.tableList)) {
                    if (skipTableRefresh) {
                        state.queryAndRenderTable().catch(console.error)
                    }
                    return
                }
                const newTableName = opts.selectTable ?? state.tableName
                if (newTableList.some((table) => table.name === newTableName)) {
                    editor.useEditorStore.getState().switchTable(newTableName)
                } else {
                    editor.useEditorStore.getState().switchTable(newTableList[0]?.name)
                }
                set({ tableList: newTableList })
            }).catch(console.error))
            await Promise.all(handles)
        },
        queryAndRenderTable: async () => {
            let state = get()
            if (state.tableName === undefined) { return }
            const { wr, type } = state.tableList.find(({ name }) => name === state.tableName) ?? {}
            if (wr === undefined || type === undefined) { return }

            // `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case.
            if (state.viewerStatement === "SELECT") {
                const records = await state.sql.query(`SELECT ${(wr || type !== "table") ? "" : "rowid AS rowid, "}* FROM ${escapeSQLIdentifier(state.tableName)} ${state.viewerConstraints} LIMIT ? OFFSET ?`, [state.paging.pageSize, (state.paging.page - 1n) * state.paging.pageSize], "r")
                const newRecordCount = (await state.sql.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(state.tableName)} ${state.viewerConstraints}`, [], "r"))[0]!.count
                if (typeof newRecordCount !== "bigint") { throw new Error(newRecordCount + "") }
                get().setPaging({ numRecords: newRecordCount })
                state = get() // state.paging will be updated
                if (state.tableName === undefined) { return }
                useTableStore.getState().update(
                    await state.sql.getTableInfo(state.tableName),
                    records,
                    (state.paging.page - 1n) * state.paging.pageSize,
                    state.tableName === null ? false : await state.sql.hasTableAutoincrement(state.tableName),
                )
            } else {
                useTableStore.getState().update(null, (await state.sql.query(`${state.viewerStatement} ${state.pragma}`, [], "r")) ?? [], 0n, false)
            }
        },
    }
})

const App = () => {
    const switchEditorTable = editor.useEditorStore((state) => state.switchTable)
    const state = useMainStore()
    useEffect(() => {
        state.queryAndRenderTable().catch(console.error)
    }, [state.viewerStatement, state.pragma, state.tableName, state.viewerConstraints, state.tableList, state.paging.page, state.paging.pageSize])

    useEffect(() => {
        const handler = ({ data }: Message) => {
            if (data.type === "sqlite3-editor-server" && data.requestId === undefined) {
                state.requireReloading()
            }
        }
        window.addEventListener("message", handler)
        return () => { window.removeEventListener("message", handler) }
    }, [])

    useEffect(() => {
        window.addEventListener("keydown", (ev) => {
            if (ev.code === "Escape") {
                const editorState = editor.useEditorStore.getState()
                if (editorState.statement === "UPDATE" || editorState.statement === "DELETE") {
                    editorState.switchTable(editorState.tableName)  // clear selections
                }
            }
        })
    }, [])

    useEffect(() => {
        const timer = setInterval(() => {
            if (useMainStore.getState().reloadRequired) {
                useMainStore.getState().reload({ refreshTableList: true })
            }
        }, 1000)
        return () => { clearInterval(timer) }
    }, [])

    return <>
        <ProgressBar />
        <h2 className="first" style={{ display: "flex" }}>
            <div style={{ whiteSpace: "pre" }}>
                <Select value={state.viewerStatement} onChange={(value) => {
                    useMainStore.setState({ viewerStatement: value })
                    if (value === "PRAGMA") {
                        switchEditorTable(undefined)
                    } else {
                        switchEditorTable(state.tableName)
                    }
                }} options={{ SELECT: {}, PRAGMA: {} }} className="primary" />
                {state.viewerStatement === "SELECT" && <> * FROM
                    {" "}
                    {state.tableName === undefined ? <>No tables</> : <Select value={state.tableName} onChange={(value) => {
                        useMainStore.setState({ tableName: value })
                        switchEditorTable(value)
                    }} options={Object.fromEntries(state.tableList.map(({ name: tableName }) => [tableName, {}] as const))} className="primary" />}
                    {" "}
                </>}
            </div>
            <div style={{ flex: 1 }}>
                {state.viewerStatement === "SELECT" && <input value={state.viewerConstraints} onBlur={(ev) => { useMainStore.setState({ viewerConstraints: ev.currentTarget.value }) }} placeholder={"WHERE <column> = <value> ORDER BY <column> ..."} autocomplete="off" style={{ width: "100%" }} />}
                {state.viewerStatement === "PRAGMA" && <Select value={state.pragma} onChange={(value) => useMainStore.setState({ pragma: value })} options={Object.fromEntries(state.pragmaList.map((k) => [k, {}]))} />}
            </div>
        </h2>
        <div>
            <div ref={state.scrollerRef} style={{ marginRight: "10px", padding: 0, maxHeight: "50vh", overflowY: "scroll", display: "inline-block", maxWidth: "100%", boxShadow: "0 0 0px 2px #000000ad" }}>
                <Table tableName={state.tableName} />
            </div>
        </div>
        <div style={{ marginBottom: "30px", paddingTop: "3px" }} className="primary">
            <span><span style={{ cursor: "pointer", paddingLeft: "8px", paddingRight: "8px", userSelect: "none" }} onClick={() => state.setPaging({ page: state.paging.page - 1n })}>‹</span><input value={"" + state.paging.page} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onChange={(ev) => state.setPaging({ page: BigInt(ev.currentTarget.value) })} /> / {state.getPageMax()} <span style={{ cursor: "pointer", paddingLeft: "4px", paddingRight: "8px", userSelect: "none" }} onClick={() => state.setPaging({ page: state.paging.page + 1n })}>›</span></span>
            <span style={{ marginLeft: "40px" }}><input value={"" + state.paging.pageSize} style={{ textAlign: "center", width: "50px", background: "white", color: "black" }} onBlur={(ev) => state.setPaging({ pageSize: BigInt(ev.currentTarget.value) })} /> records</span>
        </div>
        {state.errorMessage && <p style={{ background: "rgb(14, 72, 117)", color: "white", padding: "10px" }}>
            <pre>{state.errorMessage}</pre>
            <input type="button" value="Close" className="primary" style={{ marginTop: "10px" }} onClick={() => useMainStore.setState({ errorMessage: "" })} />
        </p>}
        <editor.Editor tableList={state.tableList} />
    </>
}

(async () => {
    const { sql } = useMainStore.getState()
    const tableList = await sql.getTableList()
    const tableName = tableList[0]?.name
    editor.useEditorStore.setState({ tableName })
    useMainStore.setState({
        tableName,
        tableList,
        pragmaList: (await sql.query("PRAGMA pragma_list", [], "r")).map(({ name }) => name as string),
    })
    render(<App />, document.body)
})().catch((err) => {
    console.error(err)
    document.write(err)
    document.write(useMainStore.getState().errorMessage)
})
