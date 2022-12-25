import { enableMapSet } from "immer"
enableMapSet()

import { render } from "preact"
import * as editor from "./editor"
import deepEqual from "fast-deep-equal"
import { useEffect, useRef, Ref } from "preact/hooks"
import * as remote from "./remote"
import { Button, persistentUseState, Select } from "./components"
import { escapeSQLIdentifier, Table, useTableStore } from "./table"
import zustand from "zustand"
import "./scrollbar"
import { SettingsView } from "./settings_view"

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

const BigintMath = {
    max: (...args: bigint[]) => args.reduce((prev, curr) => curr > prev ? curr : prev),
    min: (...args: bigint[]) => args.reduce((prev, curr) => curr < prev ? curr : prev),
}

const reloadTable = async (visibleAreaOnly: boolean) => {
    let state = useMainStore.getState()
    if (state.tableName === undefined) { return }
    const { viewerStatement } = state
    const { wr, type } = state.tableList.find(({ name }) => name === state.tableName) ?? {}
    if (wr === undefined || type === undefined) { return }
    const tableName = viewerStatement === "SELECT" ? state.tableName : `pragma_${state.pragma.toLowerCase()}`
    const tableInfo = visibleAreaOnly ? useTableStore.getState().tableInfo : await remote.getTableInfo(tableName)
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

    if (!visibleAreaOnly) {
        const numRecords = (await remote.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(tableName)}${findWidgetQuery}`, findWidgetParams, "r"))[0]!.count
        if (typeof numRecords !== "bigint") { throw new Error(numRecords + "") }
        await state.setPaging({ numRecords }, undefined, true)
        state = useMainStore.getState() // state.paging will be updated
    }

    const records = await remote.query(`SELECT${/* without rowid */!wr && type === "table" ? " rowid AS rowid," : ""} * FROM ${escapeSQLIdentifier(tableName)}${findWidgetQuery} LIMIT ? OFFSET ?`, [...findWidgetParams, state.paging.visibleAreaSize, state.paging.visibleAreaTop], "r") ?? []

    if (visibleAreaOnly) {
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
    reloadAllTables: (opts: editor.OnWriteOptions) => Promise<void>
    reloadCurrentTable: () => Promise<void>
    reloadVisibleArea: () => Promise<void>
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
            await get().reloadCurrentTable()
            if (opts.viewerStatement !== undefined || opts.tableName !== undefined) {
                await editor.useEditorStore.getState().switchTable(opts.viewerStatement === "PRAGMA" ? undefined : get().tableName)
            }
        },
        setFindWidgetState: async (opts) => {
            const oldState = get().findWidget
            const newState = { ...oldState, ...opts }
            if (deepEqual(oldState, newState)) { return }
            set({ findWidget: newState })
            await get().reloadVisibleArea()
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
                await get().reloadVisibleArea()
            }
        },
        rerender: () => set({ _rerender: {} }),
        addErrorMessage: (value: string) => set((s) => ({ errorMessage: s.errorMessage + value + "\n" })),
        reloadAllTables: async (opts: editor.OnWriteOptions) => {
            set({ reloadRequired: false })
            const state = get()
            const skipTableRefresh = opts.refreshTableList || opts.selectTable !== undefined
            const handles = new Array<Promise<void>>()
            if (!skipTableRefresh) {
                handles.push(state.reloadCurrentTable()
                    .then(() => {
                        if (opts.scrollToBottom) {
                            const state = get()
                            get().setPaging({ visibleAreaTop: BigintMath.max(state.paging.numRecords - state.paging.visibleAreaSize, 0n) })
                                .then(() => {
                                    state.scrollerRef.current?.scrollBy({ behavior: "smooth", top: get().scrollerRef.current!.scrollHeight - get().scrollerRef.current!.offsetHeight })
                                })
                                .catch(console.error)
                        }
                    })
                    .catch(console.error))
            }
            handles.push(remote.getTableList().then((newTableList) => {
                if (deepEqual(newTableList, state.tableList)) {
                    if (skipTableRefresh) {
                        state.reloadCurrentTable().catch(console.error)
                    }
                    return
                }
                let newTableName = opts.selectTable ?? state.tableName
                if (!newTableList.some((table) => table.name === newTableName)) {
                    newTableName = newTableList[0]?.name
                }
                set({ tableList: newTableList })
                get().setViewerQuery({ tableName: newTableName }).catch(console.error)
            }).catch(console.error))
            await Promise.all(handles)
        },
        reloadCurrentTable: async () => { await reloadTable(false) },
        reloadVisibleArea: async () => { await reloadTable(true) },
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
        window.addEventListener("keydown", async (ev) => {
            // only when the focus is on the input in the table
            if (!(ev.target instanceof HTMLElement && (ev.target.matches("table textarea") || !ev.target.matches("label, button, input, textarea, select, option")))) {
                return
            }

            try {
                const editorState = editor.useEditorStore.getState()
                if (editorState.statement === "UPDATE") {
                    const { tableInfo } = useTableStore.getState()
                    const { paging, setPaging } = useMainStore.getState()
                    const columnIndex = tableInfo.findIndex(({ name }) => name === editorState.column)
                    const rowNumber = Number(paging.visibleAreaTop) + editorState.row

                    const toSingleClick = () => {
                        if (!singleClick) {
                            editorState.update(editorState.tableName, editorState.column, editorState.row)
                        }
                    }
                    const moveSelectionUp = async () => {
                        if (rowNumber >= 1) {
                            if (editorState.row === 0) {
                                await setPaging({ visibleAreaTop: paging.visibleAreaTop - 1n }, true)
                                editorState.update(editorState.tableName, editorState.column, 0)
                            } else {
                                editorState.update(editorState.tableName, editorState.column, editorState.row - 1)
                            }
                        } else {
                            toSingleClick()
                        }
                    }

                    const moveSelectionDown = async () => {
                        if (rowNumber <= Number(paging.numRecords) - 2) {
                            if (editorState.row === Number(paging.visibleAreaSize) - 1) {
                                await setPaging({ visibleAreaTop: paging.visibleAreaTop + 1n }, true)
                                editorState.update(editorState.tableName, editorState.column, Number(paging.visibleAreaSize) - 1)
                            } else {
                                editorState.update(editorState.tableName, editorState.column, editorState.row + 1)
                            }
                        } else {
                            toSingleClick()
                        }
                    }

                    const moveSelectionRight = () => {
                        if (columnIndex <= tableInfo.length - 2) {
                            editorState.update(editorState.tableName, tableInfo[columnIndex + 1]!.name, editorState.row)
                        } else {
                            toSingleClick()
                        }
                    }

                    const moveSelectionLeft = () => {
                        if (columnIndex >= 1) {
                            editorState.update(editorState.tableName, tableInfo[columnIndex - 1]!.name, editorState.row)
                        } else {
                            toSingleClick()
                        }
                    }

                    const singleClick = document.querySelector(".single-click") !== null  // TODO:
                    switch (ev.code) {
                        case "Escape":
                            if (singleClick) {
                                await editorState.clearInputs()
                            } else {
                                editorState.update(editorState.tableName, editorState.column, editorState.row)
                            }
                            break
                        case "ArrowUp":
                            if (!ev.ctrlKey && !ev.shiftKey && !ev.altKey && singleClick) { await moveSelectionUp() }
                            break
                        case "ArrowDown":
                            if (!ev.ctrlKey && !ev.shiftKey && !ev.altKey && singleClick) { await moveSelectionDown() }
                            break
                        case "ArrowLeft":
                            if (!ev.ctrlKey && !ev.shiftKey && !ev.altKey && singleClick) { moveSelectionLeft() }
                            break
                        case "ArrowRight":
                            if (!ev.ctrlKey && !ev.shiftKey && !ev.altKey && singleClick) { moveSelectionRight() }
                            break
                        case "Enter":
                            if (!ev.ctrlKey && !ev.altKey) {
                                ev.preventDefault()
                                if (singleClick) {
                                    document.querySelector(".single-click")!.classList.remove("single-click")
                                } else {
                                    await editor.useEditorStore.getState().commitUpdate(true)
                                    if (ev.shiftKey) { await moveSelectionUp() } else { await moveSelectionDown() }
                                }
                            }
                            break
                        case "Tab":
                            if (!ev.ctrlKey && !ev.altKey) {
                                ev.preventDefault()
                                if (!singleClick) {
                                    await editor.useEditorStore.getState().commitUpdate(true)
                                }
                                if (ev.shiftKey) { moveSelectionLeft() } else { moveSelectionRight() }
                            }
                            break
                    }
                } else if (editorState.statement === "DELETE") {
                    switch (ev.code) {
                        case "Escape": await editorState.clearInputs(); break
                    }
                }
            } catch (err) {
                console.error(err)
            }
        })
    }, [])

    useEffect(() => {
        const timer = setInterval(() => {
            if (useMainStore.getState().reloadRequired) {
                useMainStore.getState().reloadAllTables({ refreshTableList: true })
                    .catch(console.error)
            }
        }, 1000)
        return () => { clearInterval(timer) }
    }, [])

    const [isSettingsViewOpen, setIsSettingsViewOpen] = persistentUseState("isSettingsViewOpen", false)
    const tableType = state.tableList.find(({ name }) => name === state.tableName)?.type

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
                    <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer" style={{ background: isSettingsViewOpen ? "rgba(100, 100, 100)" : "", color: isSettingsViewOpen ? "white" : "" }} onClick={() => setIsSettingsViewOpen(!isSettingsViewOpen)}>
                        <svg className="inline [width:1em] [height:1em]"><use xlinkHref={isSettingsViewOpen ? "#close" : "#settings-gear"} /></svg>
                        <span className="ml-1">{"Schema & Indices"}</span>
                    </span>
                    <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2" style={{ background: editorStatement === "ALTER TABLE" ? "rgba(100, 100, 100)" : "", color: editorStatement === "ALTER TABLE" ? "white" : "" }}
                        onClick={() => {
                            if (editorStatement === "ALTER TABLE") {
                                editor.useEditorStore.getState().cancel().catch(console.error)
                            } else if (state.tableName !== undefined) {
                                editor.useEditorStore.getState().alterTable(state.tableName, undefined).catch(console.error)
                            }
                        }}>
                        <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#edit" /></svg>
                        <span className="ml-1">{"Alter Table"}</span>
                    </span>
                    {tableType === "table" && <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2" style={{ background: editorStatement === "DROP TABLE" ? "rgba(100, 100, 100)" : "", color: editorStatement === "DROP TABLE" ? "white" : "" }}
                        onClick={() => {
                            if (editorStatement === "DROP TABLE") {
                                editor.useEditorStore.getState().cancel().catch(console.error)
                            } else if (state.tableName !== undefined) {
                                editor.useEditorStore.getState().dropTable(state.tableName)
                            }
                        }}>
                        <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#trash" /></svg>
                        <span className="ml-1">{"Drop Table"}</span>
                    </span>}
                    {tableType === "view" && <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2" style={{ background: editorStatement === "DROP VIEW" ? "rgba(100, 100, 100)" : "", color: editorStatement === "DROP VIEW" ? "white" : "" }}
                        onClick={() => {
                            if (editorStatement === "DROP VIEW") {
                                editor.useEditorStore.getState().cancel().catch(console.error)
                            } else if (state.tableName !== undefined) {
                                editor.useEditorStore.getState().dropView(state.tableName)
                            }
                        }}>
                        <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#trash" /></svg>
                        <span className="ml-1">{"Drop View"}</span>
                    </span>}
                    <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2" style={{ background: editorStatement === "CREATE TABLE" ? "rgba(100, 100, 100)" : "", color: editorStatement === "CREATE TABLE" ? "white" : "" }}
                        onClick={() => {
                            if (editorStatement === "CREATE TABLE") {
                                editor.useEditorStore.getState().cancel().catch(console.error)
                            } else {
                                editor.useEditorStore.getState().createTable(state.tableName)
                            }
                        }}>
                        <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#add" /></svg>
                        <span className="ml-1">{"Create Table"}</span>
                    </span>
                    <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2" style={{ background: editorStatement === "Custom Query" ? "rgba(100, 100, 100)" : "", color: editorStatement === "Custom Query" ? "white" : "" }}
                        onClick={() => {
                            if (editorStatement === "Custom Query") {
                                editor.useEditorStore.getState().cancel().catch(console.error)
                            } else {
                                editor.useEditorStore.getState().custom(state.tableName)
                            }
                        }}>
                        <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#terminal" /></svg>
                        <span className="ml-1">{"Custom Query"}</span>
                    </span>
                    <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2"
                        onClick={() => { alert("TODO") }}>
                        <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#tools" /></svg>
                        <span className="ml-1">{"Tools…"}</span>
                    </span>
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
            {state.errorMessage && <p className="text-white [background:rgb(14,72,117)] [padding:10px]">
                <pre className="whitespace-pre-wrap [font-size:inherit] overflow-auto h-28">{state.errorMessage}</pre>
                <Button className="primary [margin-top:10px]" onClick={() => useMainStore.setState({ errorMessage: "" })}>Close</Button>
            </p>}
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
