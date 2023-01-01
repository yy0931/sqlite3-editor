import { enableMapSet } from "immer"
enableMapSet()

import { render } from "preact"
import * as editor from "./editor"
import deepEqual from "fast-deep-equal"
import { useEffect, useRef, Ref } from "preact/hooks"
import * as remote from "./remote"
import { Button, persistentUseState, Select, SVGCheckbox } from "./components"
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

export const reloadTable = async (reloadSchema: boolean, reloadRecordCount: boolean) => {
    let state = useMainStore.getState()
    if (state.tableName === undefined) { return }
    const { viewerStatement } = state
    const { wr, type } = state.tableList.find(({ name }) => name === state.tableName) ?? {}
    if (wr === undefined || type === undefined) { return }
    const tableName = viewerStatement === "SELECT" ? state.tableName : `pragma_${state.pragma.toLowerCase()}`
    const tableInfo = reloadSchema ? await remote.getTableInfo(tableName) : useTableStore.getState().tableInfo
    let findWidgetQuery = ""
    let findWidgetParams: string[] = []
    if (state.findWidget.value) {
        if (state.findWidget.regex) {
            findWidgetQuery = tableInfo.map(({ name: column }) => `find_widget_regexp(IFNULL(${escapeSQLIdentifier(column)}, 'NULL'), ?, ${state.findWidget.wholeWord ? "1" : "0"}, ${state.findWidget.caseSensitive ? "1" : "0"})`).join(" OR ")
        } else {
            if (state.findWidget.wholeWord) {
                if (state.findWidget.caseSensitive) {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `IFNULL(${escapeSQLIdentifier(column)}, 'NULL') = ?`).join(" OR ")
                } else {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `UPPER(IFNULL(${escapeSQLIdentifier(column)}, 'NULL')) = UPPER(?)`).join(" OR ")
                }
            } else {
                if (state.findWidget.caseSensitive) {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `INSTR(IFNULL(${escapeSQLIdentifier(column)}, 'NULL'), ?) > 0`).join(" OR ")
                } else {
                    findWidgetQuery = tableInfo.map(({ name: column }) => `INSTR(UPPER(IFNULL(${escapeSQLIdentifier(column)}, 'NULL')), UPPER(?)) > 0`).join(" OR ")
                }
            }
        }
        findWidgetParams = tableInfo.map(() => state.findWidget.value)
    }
    if (findWidgetQuery !== "") {
        findWidgetQuery = ` WHERE ${findWidgetQuery}`
    }

    if (reloadRecordCount) {
        const numRecords = (await remote.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(tableName)}${findWidgetQuery}`, findWidgetParams, "r"))[0]!.count
        if (typeof numRecords !== "bigint") { throw new Error(numRecords + "") }
        await state.setPaging({ numRecords }, undefined, true)
        state = useMainStore.getState() // state.paging will be updated
    }

    const records = await remote.query(`SELECT${/* without rowid */!wr && type === "table" ? " rowid AS rowid," : ""} * FROM ${escapeSQLIdentifier(tableName)}${findWidgetQuery} LIMIT ? OFFSET ?`, [...findWidgetParams, state.paging.visibleAreaSize, state.paging.visibleAreaTop], "r") ?? []

    if (!reloadSchema) {
        useTableStore.setState({ records })
    } else {
        if (viewerStatement === "SELECT") {
            // SELECT
            const indexList = await remote.getIndexList(tableName)
            useTableStore.setState({
                tableInfo,
                records,
                autoIncrement: tableName === null ? false : await remote.hasTableAutoincrementColumn(tableName),
                indexList,
                indexInfo: await Promise.all(indexList.map((index) => remote.getIndexInfo(index.name))),
                indexSchema: await Promise.all(indexList.map((index) => remote.getIndexSchema(index.name).then((x) => x ?? null))),
                tableSchema: await remote.getTableSchema(tableName),
            })
        } else {
            // PRAGMA
            useTableStore.setState({
                tableInfo,
                records,
                autoIncrement: false,
                tableSchema: await remote.getTableSchema(tableName),
            })
        }
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
    viewerStatement: "SELECT" | "PRAGMA"  // set via setViewerQuery
    pragma: string                        // set via setViewerQuery
    tableName: string | undefined         // set via setViewerQuery
    findWidget: {  // set via setFindWidgetState
        value: string
        caseSensitive: boolean
        wholeWord: boolean
        regex: boolean
    }
    tableList: remote.TableListItem[]
    pragmaList: string[]
    scrollerRef: { current: HTMLDivElement | null }
    _rerender: Record<string, never>,
    setViewerQuery: (opts: {
        viewerStatement?: "SELECT" | "PRAGMA"
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
        viewerStatement: "SELECT",
        pragma: "analysis_limit",
        findWidget: {
            value: "",
            caseSensitive: false,
            wholeWord: false,
            regex: false,
        },
        tableList: [],
        pragmaList: [],
        scrollerRef: { current: null },
        _rerender: {},
        setViewerQuery: async (opts) => {
            set(opts)
            await remote.setState("tableName", get().tableName)
            await reloadTable(true, true)
            if (opts.viewerStatement !== undefined || opts.tableName !== undefined) {
                await editor.useEditorStore.getState().switchTable(opts.viewerStatement === "PRAGMA" ? undefined : get().tableName)
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
    const state = useMainStore(({ requireReloading, viewerStatement, tableName, errorMessage, tableList, setViewerQuery, pragma, pragmaList, setPaging }) =>
        ({ requireReloading, viewerStatement, tableName, errorMessage, tableList, setViewerQuery, pragma, pragmaList, setPaging }))

    const editorStatement = editor.useEditorStore((state) => state.statement)

    useEffect(() => {
        const handler = ({ data }: remote.Message) => {
            if (data.type === "sqlite3-editor-server" && data.requestId === undefined) {
                state.requireReloading()
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

    const [isSettingsViewOpen, setIsSettingsViewOpen] = persistentUseState("isSettingsViewOpen", false)

    return <>
        <LoadingIndicator />
        <h2 className="[padding-top:var(--page-padding)]">
            <Select value={state.viewerStatement} onChange={(value) => { state.setViewerQuery({ viewerStatement: value }).catch(console.error) }} options={{ SELECT: {}, PRAGMA: {} }} className="primary" />
            {state.viewerStatement === "SELECT" && <> * FROM
                {" "}
                {state.tableName === undefined ? <>No tables</> : <Select value={state.tableName} onChange={(value) => { state.setViewerQuery({ tableName: value }).catch(console.error) }} options={Object.fromEntries(state.tableList.map(({ name: tableName }) => [tableName, {}] as const).sort((a, b) => a[0].localeCompare(b[0])))} className="primary" />}
                {" "}
            </>}
            {state.viewerStatement === "PRAGMA" && <Select value={state.pragma} onChange={(value) => state.setViewerQuery({ pragma: value })} options={Object.fromEntries(state.pragmaList.map((k) => [k, {}]))} />}
            {state.viewerStatement === "SELECT" && (() => {
                return <div className="ml-0 block lg:ml-2 lg:inline">
                    <SVGCheckbox icon={isSettingsViewOpen ? "#close" : "#settings-gear"} checked={isSettingsViewOpen} onClick={() => setIsSettingsViewOpen(!isSettingsViewOpen)}>Schema</SVGCheckbox>
                    <SVGCheckbox icon="#add" checked={editorStatement === "CREATE TABLE"} className="ml-2" onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().createTable(state.tableName)
                    }}>Create Table</SVGCheckbox>
                    <SVGCheckbox icon="#terminal" checked={editorStatement === "Custom Query"} className="ml-2" onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().custom(state.tableName)
                    }}>Custom Query</SVGCheckbox>
                    <SVGCheckbox icon="#tools" checked={false} className="ml-2" onClick={(checked) => {
                        alert("TODO")
                    }}>Tools…</SVGCheckbox>
                </div>
            })()}
        </h2>
        {isSettingsViewOpen && <div>
            <SettingsView />
            <hr className="mt-2 border-b-2 border-b-gray-400" />
        </div>}
        {!isSettingsViewOpen && <>
            <div className="relative w-max max-w-full [padding-left:var(--page-padding)] [padding-right:var(--page-padding)]">
                <Table tableName={state.tableName} />
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
        pragmaList: (await remote.query("PRAGMA pragma_list", [], "r")).map(({ name }) => name as string),
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
