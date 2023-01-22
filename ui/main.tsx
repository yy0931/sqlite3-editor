import { enableMapSet } from "immer"
enableMapSet()

import { render } from "preact"
import * as editor from "./editor"
import deepEqual from "fast-deep-equal"
import { useEffect, useRef } from "preact/hooks"
import * as remote from "./remote"
import { Button, Highlight, persistentUseState, Select, SVGCheckbox, SVGOnlyCheckbox } from "./components"
import { escapeSQLIdentifier, Table, useTableStore } from "./table"
import "./scrollbar"
import { SettingsView } from "./schema_view"
import { onKeydown } from "./keybindings"
import { useEventListener, useInterval } from "usehooks-ts"
import { createStore } from "./util"

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
    const ref = useRef<HTMLDivElement>(null)
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
    return <div class="progressbar inline-block select-none pointer-events-none absolute top-0 z-[100] h-[5px] bg-[var(--button-primary-background)] opacity-0" ref={ref} style={{ width: width + "px", transition: "opacity 0.5s cubic-bezier(1.000, 0.060, 0.955, -0.120)" }}></div>
}

export const BigintMath = {
    max: (...args: bigint[]) => args.reduce((prev, curr) => curr > prev ? curr : prev),
    min: (...args: bigint[]) => args.reduce((prev, curr) => curr < prev ? curr : prev),
}

/** Build the WHERE clause from the state of the find widget */
const buildFindWidgetQuery = (tableInfo: remote.TableInfo) => {
    const { findWidget, isFindWidgetVisible } = useMainStore.getState()
    let findWidgetQuery = ""
    let findWidgetParams: string[] = []
    if (isFindWidgetVisible) {
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
    }
    return { findWidgetQuery, findWidgetParams }
}

/** Queries the visible area of the database table. */
export const reloadTable = async (reloadSchema: boolean, reloadRecordCount: boolean) => {
    try {
        let state = useMainStore.getState()
        const { useCustomViewerQuery, customViewerQuery } = state

        /** The table name, or a subquery if using a custom query. */
        let subquery: string
        /** True if `SELECT * FROM subquery` has row ids. */
        let hasRowId: boolean
        /** The list of columns of `SELECT * FROM subquery`. */
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

        // Filter records by the state of the find widget
        const { findWidgetQuery, findWidgetParams } = buildFindWidgetQuery(tableInfo)
        if (reloadRecordCount) {
            const numRecords = (await remote.query(`SELECT COUNT(*) as count FROM ${subquery}${findWidgetQuery}`, findWidgetParams, "r", { withoutLogging: true })).records[0]!.count
            if (typeof numRecords !== "bigint") { throw new Error(numRecords + "") }
            await state.setPaging({ numRecords }, undefined, true)
            state = useMainStore.getState() // state.paging will be updated
        }

        // Query the database
        const records = (await remote.query(`SELECT${hasRowId ? " rowid AS rowid," : ""} * FROM ${subquery}${findWidgetQuery} LIMIT ? OFFSET ?`, [...findWidgetParams, state.paging.visibleAreaSize, state.paging.visibleAreaTop], "r", { withoutLogging: true })).records

        // Update the store
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

export const useMainStore = createStore({
    reloadRequired: false,
    isConfirmDialogVisible: false as false | ((value: boolean) => void),
    tableName: undefined as string | undefined,
    paging: {
        visibleAreaTop: 0n,
        numRecords: 0n,
        visibleAreaSize: 20n,
    },
    errorMessage: "",
    useCustomViewerQuery: false,
    customViewerQuery: "",
    findWidget: {
        value: "",
        caseSensitive: false,
        wholeWord: false,
        regex: false,
    },
    isFindWidgetVisible: false,
    tableList: [] as remote.TableListItem[],
    autoReload: true,
    _rerender: {} as Record<string, never>,
}, (set, get) => {
    const setViewerQuery = async (opts: {
        useCustomViewerQuery?: boolean
        customViewerQuery?: string
        tableName?: string
    }) => {
        set(opts)
        await remote.setState("tableName", get().tableName)
        await reloadTable(true, true)
        if (opts.useCustomViewerQuery !== undefined || opts.customViewerQuery !== undefined || opts.tableName !== undefined) {
            await editor.useEditorStore.getState().switchTable(opts.useCustomViewerQuery ? undefined : get().tableName)
        }
    }
    return {
        setViewerQuery,
        /** Displays `confirm("Commit changes?")` using a `<dialog>`. */
        confirm: async (): Promise<boolean> => {
            return new Promise<boolean>((resolve) => {
                set({
                    isConfirmDialogVisible: (value) => {
                        set({ isConfirmDialogVisible: false })
                        resolve(value)
                    }
                })
            })
        },
        getPageMax: () => BigInt(Math.ceil(Number(get().paging.numRecords) / Number(get().paging.visibleAreaSize))),
        setFindWidgetVisibility: async (value: boolean) => {
            if (get().isFindWidgetVisible === value) { return }
            set({ isFindWidgetVisible: value })
            await reloadTable(false, true)
        },
        setFindWidgetState: async (opts: {
            value?: string
            caseSensitive?: boolean
            wholeWord?: boolean
            regex?: boolean
        }) => {
            const oldState = get().findWidget
            const newState = { ...oldState, ...opts }
            if (deepEqual(oldState, newState)) { return }
            set({ findWidget: newState })
            await reloadTable(false, true)
        },
        requireReloading: () => { set({ reloadRequired: true }) },
        setPaging: async (opts: { visibleAreaTop?: bigint, numRecords?: bigint, visibleAreaSize?: bigint }, preserveEditorState?: true, withoutTableReloading?: true) => {
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
                await setViewerQuery({ tableName: newTableName })
            } else {
                // If the list of tables is not changed
                await reloadTable(true, true)
            }
        },
    }
})

/** The root element. */
const App = () => {
    const requireReloading = useMainStore((s) => s.requireReloading)
    const isConfirmationDialogVisible = useMainStore((s) => s.isConfirmDialogVisible)
    const errorMessage = useMainStore((s) => s.errorMessage)
    const tableList = useMainStore((s) => s.tableList)
    const setViewerQuery = useMainStore((s) => s.setViewerQuery)
    const setPaging = useMainStore((s) => s.setPaging)
    const isFindWidgetVisible = useMainStore((s) => s.isFindWidgetVisible)
    const autoReload = useMainStore((s) => s.autoReload)
    const useCustomViewerQuery = useMainStore((s) => s.useCustomViewerQuery)
    const customViewerQuery = useMainStore((s) => s.customViewerQuery)
    const tableName = useMainStore((s) => s.tableName)
    const tableType = useMainStore((s) => s.tableList.find(({ name }) => name === tableName)?.type)
    const editorStatement = editor.useEditorStore((s) => s.statement)
    const isTableRendered = useTableStore((s) => s.invalidQuery === null)
    const [isSettingsViewOpen, setIsSettingsViewOpen] = persistentUseState("isSettingsViewOpen", false)
    const confirmDialogRef = useRef<HTMLDialogElement>(null)

    // Show or close the confirmation dialog
    useEffect(() => {
        if (isConfirmationDialogVisible) {
            confirmDialogRef.current?.showModal()
        } else {
            confirmDialogRef.current?.close()
        }
    }, [isConfirmationDialogVisible])
    useEventListener("close", () => {
        const { isConfirmDialogVisible } = useMainStore.getState()
        if (isConfirmDialogVisible) {
            isConfirmDialogVisible(false)
        }
    }, confirmDialogRef)

    // Reload all tables if the database file is updated
    useEventListener("message", ({ data }: remote.Message) => {
        if (data.type === "sqlite3-editor-server" && data.requestId === undefined) {
            if (useMainStore.getState().autoReload) {
                requireReloading()
            }
        }
    })
    useInterval(() => {
        if (useMainStore.getState().reloadRequired) {
            useMainStore.getState().reloadAllTables()
                .catch(console.error)
        }
    }, 1000)

    // Register keyboard shortcuts
    useEventListener("keydown", onKeydown)

    return <>
        <LoadingIndicator />

        {/* Header `SELECT * FROM ...` */}
        <h2 class="pt-[var(--page-padding)]">
            <div class="mb-2">
                {/* The buttons placed at the top-right corner */}
                <div class="mb-2 float-right">
                    {/* Create Table button */}
                    <SVGCheckbox icon="#empty-window" checked={editorStatement === "CREATE TABLE"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().createTable(tableName)
                    }}>Create Table</SVGCheckbox>

                    {/* Custom Query button */}
                    <SVGCheckbox icon="#terminal" checked={editorStatement === "Custom Query"} class="ml-2" onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().custom(tableName)
                    }}>Custom Query</SVGCheckbox>
                </div>

                {/* SELECT * FROM ... */}
                {!useCustomViewerQuery && <>
                    <Highlight>SELECT </Highlight>
                    *
                    <Highlight> FROM </Highlight>
                    {tableName === undefined ? <>No tables</> : <Select value={tableName} onChange={(value) => { setViewerQuery({ tableName: value }).catch(console.error) }} options={Object.fromEntries(tableList.map(({ name: tableName, type }) => [tableName, { group: type }] as const).sort((a, b) => a[0].localeCompare(b[0])))} class="primary" />}
                </>}

                {/* Custom Query */}
                {useCustomViewerQuery && <>
                    <input placeholder="SELECT * FROM table-name" class="w-96" value={customViewerQuery} onBlur={(ev) => { setViewerQuery({ customViewerQuery: ev.currentTarget.value }).catch(console.error) }}></input>
                </>}

                {/* Buttons placed right after the table name */}
                <span class="ml-1">
                    {/* Schema */}
                    {!useCustomViewerQuery && <SVGOnlyCheckbox icon={isSettingsViewOpen ? "#close" : "#settings-gear"} title="Schema" checked={isSettingsViewOpen} onClick={() => setIsSettingsViewOpen(!isSettingsViewOpen)}></SVGOnlyCheckbox>}

                    {/* Drop Table */}
                    {!useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#trash" title="Drop Table" checked={editorStatement === "DROP TABLE"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().dropTable(tableName)
                    }}></SVGOnlyCheckbox>}

                    {/* Drop View */}
                    {!useCustomViewerQuery && tableName && tableType === "view" && <SVGOnlyCheckbox icon="#trash" title="Drop View" checked={editorStatement === "DROP VIEW"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().dropView(tableName)
                    }}></SVGOnlyCheckbox>}

                    {/* Alter Table */}
                    {!useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#edit" title="Alter Table" checked={editorStatement === "ALTER TABLE"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().alterTable(tableName, undefined).catch(console.error)
                    }}></SVGOnlyCheckbox>}

                    {/* Create Index */}
                    {!useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#symbol-interface" title="Create Index" checked={editorStatement === "CREATE INDEX"} onClick={(checked) => {
                        if (!checked) { editor.useEditorStore.getState().cancel().catch(console.error); return }
                        editor.useEditorStore.getState().createIndex(tableName)
                    }}></SVGOnlyCheckbox>}

                    {/* Find */}
                    {isTableRendered && !isSettingsViewOpen && <SVGOnlyCheckbox icon="#search" title="Find" checked={isFindWidgetVisible} onClick={(checked) => {
                        useMainStore.getState().setFindWidgetVisibility(checked).catch(console.error)
                    }}></SVGOnlyCheckbox>}
                </span>

                {/* The checkbox to toggle the custom query mode */}
                <label class="ml-2 select-none cursor-pointer"><input type="checkbox" checked={useCustomViewerQuery} onChange={() => { setViewerQuery({ useCustomViewerQuery: !useCustomViewerQuery }).catch(console.error) }}></input> Custom</label>

                {/* The checkbox to toggle auto reloading */}
                <label class="select-none cursor-pointer ml-2" title="Reload the table when the database is updated."><input type="checkbox" checked={autoReload} onChange={() => { useMainStore.setState({ autoReload: !autoReload }) }}></input> Auto reload</label>
            </div>
        </h2>

        {/* Schema and Index */}
        {isSettingsViewOpen && <div>
            <SettingsView />
            <hr class="mt-2 border-b-2 border-b-gray-400" />
        </div>}

        {/* Table */}
        {!isSettingsViewOpen && <>
            <div class="relative w-max max-w-full pl-[var(--page-padding)] pr-[var(--page-padding)]">
                <Table tableName={tableName} />
            </div>

            {/* The horizontal handle to resize the height of the table */}
            <div class="h-2 cursor-ns-resize select-none" onMouseDown={(ev) => {
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
                    setPaging({ visibleAreaSize: useMainStore.getState().paging.visibleAreaSize + pageSizeDelta })
                        .catch(console.error)
                }
                window.addEventListener("mousemove", onMouseMove)
                window.addEventListener("mouseup", () => {
                    window.removeEventListener("mousemove", onMouseMove)
                    document.body.classList.remove("ns-resize")
                }, { once: true })
            }}>
                <hr class="mt-2 border-b-2 border-b-gray-400" />
            </div>
        </>}

        {/* Error Message */}
        {errorMessage && <p class="text-white bg-[rgb(14,72,117)] [padding:10px]">
            <pre class="whitespace-pre-wrap [font-size:inherit] overflow-auto h-28">{errorMessage}</pre>
            <Button class="primary mt-[10px]" onClick={() => useMainStore.setState({ errorMessage: "" })}>Close</Button>
        </p>}

        {/* Editor */}
        <editor.Editor />

        {/* Confirmation Dialog */}
        <dialog class="p-4 bg-[#f0f0f0] shadow-2xl mx-auto mt-[10vh]" ref={confirmDialogRef}>
            <p class="pb-2">Commit changes?</p>
            <Button onClick={() => { if (isConfirmationDialogVisible) { isConfirmationDialogVisible(true) } }} class="mr-1">Commit</Button>
            <Button onClick={() => { if (isConfirmationDialogVisible) { isConfirmationDialogVisible(false) } }} class="bg-[var(--dropdown-background)] hover:[background-color:#8e8e8e]">Discard changes</Button>
        </dialog>
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
    useMainStore.setState({ tableList })
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
