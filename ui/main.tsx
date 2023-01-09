import { enableMapSet } from "immer"
enableMapSet()

import { render } from "preact"
import * as editor from "./editor"
import deepEqual from "fast-deep-equal"
import { useEffect, useRef, Ref } from "preact/hooks"
import * as remote from "./remote"
import { Button, Highlight, persistentUseState, Select, SVGCheckbox, SVGOnlyCheckbox } from "./components"
import { escapeSQLIdentifier, Table, useTableStore } from "./table"
import zustand from "zustand"
import "./scrollbar"
import { SettingsView } from "./schema_view"
import { onKeydown } from "./keybindings"

export type VSCodeAPI = {
    postMessage(data: unknown): void
    getState(): unknown
    setState(value: unknown): void
}

declare global {
    interface Window {
        acquireVsCodeApi?: () => VSCodeAPI
    }
}

/** Looping animation to indicate loading state, visible only when body.querying. */
const LoadingIndicator = () => {
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
                ref.current!.style.opacity = "1"
            } else {
                ref.current!.style.opacity = "0"
            }
            requestAnimationFrame(loop)
        }
        loop()
        return () => { canceled = true }
    }, [])
    return <div className="progressbar inline-block select-none pointer-events-none absolute top-0 [z-index:100] [height:5px] [background:var(--button-primary-background)] opacity-0" ref={ref} style={{ width: width + "px", transition: "opacity 0.5s cubic-bezier(1.000, 0.060, 0.955, -0.120)" }}></div>
}

export const BigintMath = {
    max: (...args: bigint[]) => args.reduce((prev, curr) => curr > prev ? curr : prev),
    min: (...args: bigint[]) => args.reduce((prev, curr) => curr < prev ? curr : prev),
}

const buildFindWidgetQuery = (tableInfo: remote.TableInfo) => {
    const { findWidget } = useMainStore.getState()
    let findWidgetQuery = ""
    let findWidgetParams: string[] = []
    if (findWidget.value) {
        if (findWidget.regex) {
            findWidgetQuery = tableInfo.map(({ name: column }) => `find_widget_regexp(IFNULL(${escapeSQLIdentifier(column)}, 'NULL'), ?, ${findWidget.wholeWord ? "1" : "0"}, ${findWidget.caseSensitive ? "1" : "0"})`).join(" OR ")
        } else {
            if (findWidget.wholeWord) {
                if (findWidget.caseSensitive) {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `IFNULL(${escapeSQLIdentifier(column)}, 'NULL') = ?`).join(" OR ")
                } else {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `UPPER(IFNULL(${escapeSQLIdentifier(column)}, 'NULL')) = UPPER(?)`).join(" OR ")
                }
            } else {
                if (findWidget.caseSensitive) {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `INSTR(IFNULL(${escapeSQLIdentifier(column)}, 'NULL'), ?) > 0`).join(" OR ")
                } else {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `INSTR(UPPER(IFNULL(${escapeSQLIdentifier(column)}, 'NULL')), UPPER(?)) > 0`).join(" OR ")
                }
            }
        }
        findWidgetParams = tableInfo.map(() => findWidget.value)
    }
    if (findWidgetQuery !== "") {
        findWidgetQuery = ` WHERE ${findWidgetQuery}`
    }
    return { findWidgetQuery, findWidgetParams }
}

export const reloadTable = async (reloadSchema: boolean, reloadRecordCount: boolean) => {
    try {
        let state = useMainStore.getState()
        const { useCustomViewerQuery, customViewerQuery } = state

        let subquery: string
        let hasRowId: boolean
        let tableInfo: remote.TableInfo
        if (!useCustomViewerQuery) {
            if (state.tableName === undefined) { useTableStore.setState({ invalidQuery: "" }); return }
            subquery = escapeSQLIdentifier(state.tableName)

            const tableListItem = state.tableList.find(({ name }) => name === state.tableName!)
            if (tableListItem === undefined) { useTableStore.setState({ invalidQuery: "" }); return }
            hasRowId = tableListItem.type === "table" && !tableListItem.wr
            tableInfo = !reloadSchema ? useTableStore.getState().tableInfo : await remote.getTableInfo(state.tableName, { withoutLogging: true })
        } else {
            if (customViewerQuery.trim() === "") { useTableStore.setState({ invalidQuery: "" }); return }
            subquery = `(${customViewerQuery})`
            hasRowId = false
            tableInfo = !reloadSchema ? useTableStore.getState().tableInfo : (await remote.query(`SELECT * FROM ${subquery} LIMIT 0`, [], "r", { withoutLogging: true })).columns.map((column, i): remote.TableInfo[number] => ({ name: column, cid: BigInt(i), dflt_value: null, notnull: 0n, pk: 0n, type: "" }))
        }

        const { findWidgetQuery, findWidgetParams } = buildFindWidgetQuery(tableInfo)
        if (reloadRecordCount) {
            const numRecords = (await remote.query(`SELECT COUNT(*) as count FROM ${subquery}${findWidgetQuery}`, findWidgetParams, "r", { withoutLogging: true })).records[0]!.count
            if (typeof numRecords !== "bigint") { throw new Error(numRecords + "") }
            await state.setPaging({ numRecords }, undefined, true)
            state = useMainStore.getState() // state.paging will be updated
        }

        const records = (await remote.query(`SELECT${hasRowId ? " rowid AS rowid," : ""} * FROM ${subquery}${findWidgetQuery} LIMIT ? OFFSET ?`, [...findWidgetParams, state.paging.visibleAreaSize, state.paging.visibleAreaTop], "r", { withoutLogging: true })).records

        if (!reloadSchema) {
            useTableStore.setState({ records })
        } else {
            if (!useCustomViewerQuery) {
                const indexList = await remote.getIndexList(state.tableName!, { withoutLogging: true })
                const autoIncrement = await remote.hasTableAutoincrementColumn(state.tableName!, { withoutLogging: true })
                const tableSchema = await remote.getTableSchema(state.tableName!, { withoutLogging: true })
                useTableStore.setState({
                    invalidQuery: null,
                    tableInfo,
                    records,
                    autoIncrement,
                    tableSchema,
                    indexList,
                    indexInfo: await Promise.all(indexList.map((index) => remote.getIndexInfo(index.name, { withoutLogging: true }))),
                    indexSchema: await Promise.all(indexList.map((index) => remote.getIndexSchema(index.name, { withoutLogging: true }).then((x) => x ?? null))),
                })
            } else {
                useTableStore.setState({
                    invalidQuery: null,
                    tableInfo,
                    records,
                    autoIncrement: false,
                    tableSchema: null,
                    indexList: [],
                    indexInfo: [],
                    indexSchema: [],
                })
            }
        }
    } catch (err) {
        useTableStore.setState({ invalidQuery: typeof err === "object" && err !== null && "message" in err ? err.message + "" : err + "" })
        throw err
    }
}

export const useMainStore = zustand<{
    reloadRequired: boolean
    paging: {
        visibleAreaTop: bigint
        numRecords: bigint
        visibleAreaSize: bigint
    }
    errorMessage: string
    useCustomViewerQuery: boolean
    customViewerQuery: string
    pragma: string                        // set via setViewerQuery
    tableName: string | undefined         // set via setViewerQuery
    findWidget: {  // set via setFindWidgetState
        value: string
        caseSensitive: boolean
        wholeWord: boolean
        regex: boolean
    }
    isFindWidgetVisibleWhenValueIsEmpty: boolean
    tableList: remote.TableListItem[]
    pragmaList: string[]
    scrollerRef: { current: HTMLDivElement | null }
    autoReload: boolean
    _rerender: Record<string, never>,
    setViewerQuery: (opts: {
        useCustomViewerQuery?: boolean
        customViewerQuery?: string
        pragma?: string
        tableName?: string
    }) => Promise<void>,
    setFindWidgetState: (opts: {
        value?: string
        caseSensitive?: boolean
        wholeWord?: boolean
        regex?: boolean
    }) => Promise<void>
    requireReloading: () => void
    rerender: () => void,
    getPageMax: () => bigint
    setPaging: (opts: { visibleAreaTop?: bigint, numRecords?: bigint, visibleAreaSize?: bigint }, preserveEditorState?: true, withoutTableReloading?: true) => Promise<void>
    reloadAllTables: (selectTable?: string) => Promise<void>
    addErrorMessage: (value: string) => void
}>()((set, get) => {
    return {
        reloadRequired: false,
        getPageMax: () => BigInt(Math.ceil(Number(get().paging.numRecords) / Number(get().paging.visibleAreaSize))),
        tableName: undefined,
        paging: {
            visibleAreaTop: 0n,
            numRecords: 0n,
            visibleAreaSize: 20n,
        },
        errorMessage: "",
        useCustomViewerQuery: false,
        customViewerQuery: "",
        pragma: "analysis_limit",
        findWidget: {
            value: "",
            caseSensitive: false,
            wholeWord: false,
            regex: false,
        },
        isFindWidgetVisibleWhenValueIsEmpty: false,
        tableList: [],
        pragmaList: [],
        scrollerRef: { current: null },
        autoReload: true,
        _rerender: {},
        setViewerQuery: async (opts) => {
            set(opts)
            await remote.setState("tableName", get().tableName)
            await reloadTable(true, true)
            if (opts.useCustomViewerQuery !== undefined || opts.customViewerQuery !== undefined || opts.tableName !== undefined) {
                await editor.useEditorStore.getState().switchTable(opts.useCustomViewerQuery ? undefined : get().tableName)
            }
        },
        setFindWidgetState: async (opts) => {
            const oldState = get().findWidget
            const newState = { ...oldState, ...opts }
            if (deepEqual(oldState, newState)) { return }
            set({ findWidget: newState })
            await reloadTable(false, true)
        },
        requireReloading: () => { set({ reloadRequired: true }) },
        setPaging: async (opts, preserveEditorState, withoutTableReloading) => {
            const paging = { ...get().paging, ...opts }
            paging.visibleAreaSize = BigintMath.max(1n, BigintMath.min(200n, paging.visibleAreaSize))
            paging.visibleAreaTop = BigintMath.max(0n, BigintMath.min(paging.numRecords - paging.visibleAreaSize, paging.visibleAreaTop))
            if (deepEqual(get().paging, paging)) { return }

            await remote.setState("visibleAreaSize", Number(paging.visibleAreaSize))
            await editor.useEditorStore.getState().commitUpdate()
            if (!preserveEditorState) { await editor.useEditorStore.getState().clearInputs() }
            paging.visibleAreaTop = BigintMath.max(0n, BigintMath.min(paging.numRecords - paging.visibleAreaSize, paging.visibleAreaTop))
            set({ paging })
            if (!withoutTableReloading) {
                await reloadTable(false, false)
            }
        },
        rerender: () => set({ _rerender: {} }),
        addErrorMessage: (value: string) => set((s) => ({ errorMessage: s.errorMessage + value + "\n" })),
        reloadAllTables: async (selectTable?: string) => {
            set({ reloadRequired: false })
            const state = get()

            // List tables
            const newTableList = await remote.getTableList()

            if (!deepEqual(newTableList, state.tableList)) {
                // If the list of tables is changed
                let newTableName = selectTable ?? state.tableName
                if (!newTableList.some((table) => table.name === newTableName)) {
                    newTableName = newTableList[0]?.name
                }
                set({ tableList: newTableList })
                await get().setViewerQuery({ tableName: newTableName })
            } else {
                // If the list of tables is not changed
                await reloadTable(true, true)
            }
        },
    }
})

const App = () => {
    const state = useMainStore(({ requireReloading, errorMessage, tableList, setViewerQuery, pragma, pragmaList, setPaging, isFindWidgetVisible, autoReload, useCustomViewerQuery, customViewerQuery }) =>
        ({ requireReloading, errorMessage, tableList, setViewerQuery, pragma, pragmaList, setPaging, isFindWidgetVisible, autoReload, useCustomViewerQuery, customViewerQuery }))
    const tableName = useMainStore(({ tableName }) => tableName)
    const editorStatement = editor.useEditorStore((state) => state.statement)
    const [isSettingsViewOpen, setIsSettingsViewOpen] = persistentUseState("isSettingsViewOpen", false)
    const isTableRendered = useTableStore((state) => state.invalidQuery === null)
    const tableType = useMainStore((state) => state.tableList.find(({ name }) => name === tableName)?.type)

    useEffect(() => {
        const handler = ({ data }: remote.Message) => {
            if (data.type === "sqlite3-editor-server" && data.requestId === undefined) {
                if (useMainStore.getState().autoReload) {
                    state.requireReloading()
                }
            }
        }
        window.addEventListener("message", handler)
        return () => { window.removeEventListener("message", handler) }
    }, [])

    useEffect(() => {
        window.addEventListener("keydown", onKeydown)
        return () => { window.removeEventListener("keydown", onKeydown) }
    }, [])

    useEffect(() => {
        const timer = setInterval(() => {
            if (useMainStore.getState().reloadRequired) {
                useMainStore.getState().reloadAllTables()
                    .catch(console.error)
            }
        }, 1000)
        return () => { clearInterval(timer) }
    }, [])

    return <>
        <LoadingIndicator />
        <h2 className="[padding-top:var(--page-padding)]">
            <div className="mb-2">
                <div className="mb-2 float-right">
                    <SVGCheckbox icon="#empty-window" checked={editorStatement === "CREATE TABLE"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().createTable(tableName)
                    }}>Create Table</SVGCheckbox>
                    <SVGCheckbox icon="#terminal" checked={editorStatement === "Custom Query"} className="ml-2" onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().custom(tableName)
                    }}>Custom Query</SVGCheckbox>
                </div>
                {!state.useCustomViewerQuery && <>
                    <Highlight>SELECT </Highlight>
                    *
                    <Highlight> FROM </Highlight>
                    {tableName === undefined ? <>No tables</> : <Select value={tableName} onChange={(value) => { state.setViewerQuery({ tableName: value }).catch(console.error) }} options={Object.fromEntries(state.tableList.map(({ name: tableName, type }) => [tableName, { group: type }] as const).sort((a, b) => a[0].localeCompare(b[0])))} className="primary" />}
                </>}
                {state.useCustomViewerQuery && <>
                    <input placeholder="SELECT * FROM table-name" className="w-96" value={state.customViewerQuery} onBlur={(ev) => { state.setViewerQuery({ customViewerQuery: ev.currentTarget.value }).catch(console.error) }}></input>
                </>}
                <span className="ml-1">
                    {!state.useCustomViewerQuery && <SVGOnlyCheckbox icon={isSettingsViewOpen ? "#close" : "#settings-gear"} title="Schema" checked={isSettingsViewOpen} onClick={() => setIsSettingsViewOpen(!isSettingsViewOpen)}></SVGOnlyCheckbox>}
                    {!state.useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#trash" title="Drop Table" checked={editorStatement === "DROP TABLE"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().dropTable(tableName)
                    }}></SVGOnlyCheckbox>}
                    {!state.useCustomViewerQuery && tableName && tableType === "view" && <SVGOnlyCheckbox icon="#trash" title="Drop View" checked={editorStatement === "DROP VIEW"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().dropView(tableName)
                    }}></SVGOnlyCheckbox>}
                    {!state.useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#edit" title="Alter Table" checked={editorStatement === "ALTER TABLE"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().alterTable(tableName, undefined).catch(console.error)
                    }}></SVGOnlyCheckbox>}
                    {!state.useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#symbol-interface" title="Create Index" checked={editorStatement === "CREATE INDEX"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().createIndex(tableName)
                    }}></SVGOnlyCheckbox>}
                    {isTableRendered && !isSettingsViewOpen && <SVGOnlyCheckbox icon="#search" title="Find" checked={state.isFindWidgetVisible} onClick={(checked) => {
                        useMainStore.setState({ isFindWidgetVisible: checked })
                    }}></SVGOnlyCheckbox>}
                </span>
                <label className="ml-2 select-none cursor-pointer"><input type="checkbox" checked={state.useCustomViewerQuery} onChange={() => { state.setViewerQuery({ useCustomViewerQuery: !state.useCustomViewerQuery }).catch(console.error) }}></input> Custom</label>
                <label className="select-none cursor-pointer ml-2" title="Reload the table when the database is updated."><input type="checkbox" checked={state.autoReload} onChange={() => { useMainStore.setState({ autoReload: !state.autoReload }) }}></input> Auto reload</label>
            </div>
        </h2>
        {isSettingsViewOpen && <div>
            <SettingsView />
            <hr className="mt-2 border-b-2 border-b-gray-400" />
        </div>}
        {!isSettingsViewOpen && <>
            <div className="relative w-max max-w-full [padding-left:var(--page-padding)] [padding-right:var(--page-padding)]">
                <Table tableName={tableName} />
            </div>
            <div className="h-2 cursor-ns-resize select-none" onMouseDown={(ev) => {
                ev.preventDefault()
                document.body.classList.add("ns-resize")
                let prev = ev.pageY
                const onMouseMove = (ev: MouseEvent) => {
                    const trHeight = 18  // TODO: measure the height of a tr
                    let pageSizeDelta = 0n
                    while (ev.pageY - prev > trHeight) {
                        pageSizeDelta += 1n
                        prev += trHeight
                    }
                    while (ev.pageY - prev < -trHeight) {
                        pageSizeDelta -= 1n
                        prev -= trHeight
                    }
                    state.setPaging({ visibleAreaSize: useMainStore.getState().paging.visibleAreaSize + pageSizeDelta })
                        .catch(console.error)
                }
                window.addEventListener("mousemove", onMouseMove)
                window.addEventListener("mouseup", () => {
                    window.removeEventListener("mousemove", onMouseMove)
                    document.body.classList.remove("ns-resize")
                }, { once: true })
            }}>
                <hr className="mt-2 border-b-2 border-b-gray-400" />
            </div>
        </>}
        {state.errorMessage && <p className="text-white [background:rgb(14,72,117)] [padding:10px]">
            <pre className="whitespace-pre-wrap [font-size:inherit] overflow-auto h-28">{state.errorMessage}</pre>
            <Button className="primary [margin-top:10px]" onClick={() => useMainStore.setState({ errorMessage: "" })}>Close</Button>
        </p>}
        <editor.Editor tableList={state.tableList} />
    </>
}

(async () => {
    await remote.downloadState()
    const tableList = await remote.getTableList()
    const tableName = (() => {
        const restored = remote.getState<string>("tableName")
        return restored && tableList.some(({ name }) => name === restored) ?
            restored :
            tableList[0]?.name
    })()
    useMainStore.setState({
        tableList,
        pragmaList: (await remote.query("PRAGMA pragma_list", [], "r")).records.map(({ name }) => name as string),
    })
    await useMainStore.getState().setViewerQuery({ tableName })
    {
        const restored = remote.getState<number>("visibleAreaSize")
        await useMainStore.getState().setPaging({ visibleAreaSize: restored === undefined ? undefined : BigInt(restored) })
    }
    await editor.useEditorStore.getState().switchTable(tableName)
    render(<App />, document.body)
})().catch((err) => {
    console.error(err)
    document.write(err)
    document.write(useMainStore.getState().errorMessage)
})
