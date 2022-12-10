import { enableMapSet } from "immer"
enableMapSet()

import { render } from "preact"
import * as editor from "./editor"
import deepEqual from "fast-deep-equal"
import { useEffect, useRef, Ref } from "preact/hooks"
import * as remote from "./remote"
import { Button, Select } from "./components"
import { escapeSQLIdentifier, Table, useTableStore } from "./table"
import zustand from "zustand"
import "./scrollbar"

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
            }
            requestAnimationFrame(loop)
        }
        loop()
        return () => { canceled = true }
    }, [])
    return <div className="progressbar inline-block select-none pointer-events-none absolute top-0" ref={ref} style={{ zIndex: 100, width: width + "px", height: "5px", background: "var(--button-primary-background)" }}></div>
}

const BigintMath = {
    max: (...args: bigint[]) => args.reduce((prev, curr) => curr > prev ? curr : prev),
    min: (...args: bigint[]) => args.reduce((prev, curr) => curr < prev ? curr : prev),
}

export const useMainStore = zustand<{
    reloadRequired: boolean
    paging: {
        visibleAreaTop: bigint
        numRecords: bigint
        pageSize: bigint
    }
    errorMessage: string
    viewerStatement: "SELECT" | "PRAGMA"  // set via setViewerQuery
    pragma: string                        // set via setViewerQuery
    viewerConstraints: string             // set via setViewerQuery
    tableName: string | undefined         // set via setViewerQuery
    tableList: remote.TableListItem[]
    pragmaList: string[]
    scrollerRef: { current: HTMLDivElement | null }
    _rerender: {},
    setViewerQuery: (opts: {
        viewerStatement?: "SELECT" | "PRAGMA"
        pragma?: string
        tableName?: string
        viewerConstraints?: string
    }) => Promise<void>,
    requireReloading: () => void
    rerender: () => void,
    getPageMax: () => bigint
    setPaging: (opts: { visibleAreaTop?: bigint, numRecords?: bigint, pageSize?: bigint }, preserveEditorState?: true) => Promise<void>
    reloadAllTables: (opts: editor.OnWriteOptions) => Promise<void>
    reloadCurrentTable: () => Promise<void>
    reloadVisibleArea: () => Promise<void>
    addErrorMessage: (value: string) => void
}>()((set, get) => {
    return {
        reloadRequired: false,
        getPageMax: () => BigInt(Math.ceil(Number(get().paging.numRecords) / Number(get().paging.pageSize))),
        tableName: undefined,
        paging: {
            visibleAreaTop: 0n,
            numRecords: 0n,
            pageSize: 20n,
        },
        errorMessage: "",
        viewerStatement: "SELECT",
        pragma: "analysis_limit",
        viewerConstraints: "",
        tableList: [],
        pragmaList: [],
        scrollerRef: { current: null },
        _rerender: {},
        setViewerQuery: async (opts) => {
            set(opts)
            if (opts.viewerStatement !== undefined || opts.tableName !== undefined || opts.viewerConstraints !== undefined) {  // TODO: this block should be located after reloadCurrentTable()
                await editor.useEditorStore.getState().switchTable(opts.viewerStatement === "PRAGMA" ? undefined : get().tableName)
            }
            await get().reloadCurrentTable()
        },
        requireReloading: () => { set({ reloadRequired: true }) },
        setPaging: async (opts, preserveEditorState) => {
            const paging = { ...get().paging, ...opts }
            if (deepEqual(get().paging, paging)) { return }

            await editor.useEditorStore.getState().commitUpdate()
            if (!preserveEditorState) { editor.useEditorStore.getState().clearInputs() }
            paging.visibleAreaTop = BigintMath.max(0n, BigintMath.min(paging.numRecords - paging.pageSize, paging.visibleAreaTop))
            set({ paging })
            await get().reloadVisibleArea()
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
                            get().setPaging({ visibleAreaTop: BigintMath.max(state.paging.numRecords - state.paging.pageSize, 0n) })
                                .then(() => {
                                    state.scrollerRef.current?.scrollBy({ behavior: "smooth", top: get().scrollerRef.current!.scrollHeight - get().scrollerRef.current!.offsetHeight })
                                })
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
                get().setViewerQuery({ tableName: newTableName })
            }).catch(console.error))
            await Promise.all(handles)
        },
        reloadCurrentTable: async () => {
            let state = get()
            if (state.tableName === undefined) { return }
            const { wr, type } = state.tableList.find(({ name }) => name === state.tableName) ?? {}
            if (wr === undefined || type === undefined) { return }

            // `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case.
            if (state.viewerStatement === "SELECT") {
                const records = await remote.query(`SELECT ${(wr || type !== "table") ? "" : "rowid AS rowid, "}* FROM ${escapeSQLIdentifier(state.tableName)} ${state.viewerConstraints} LIMIT ? OFFSET ?`, [state.paging.pageSize, state.paging.visibleAreaTop], "r")
                const numRecords = (await remote.query(`SELECT COUNT(*) as count FROM ${escapeSQLIdentifier(state.tableName)} ${state.viewerConstraints}`, [], "r"))[0]!.count
                if (typeof numRecords !== "bigint") { throw new Error(numRecords + "") }
                await get().setPaging({ numRecords })
                state = get() // state.paging will be updated
                if (state.tableName === undefined) { return }
                const indexList = await remote.getIndexList(state.tableName)
                useTableStore.setState({
                    tableInfo: await remote.getTableInfo(state.tableName),
                    indexList,
                    indexInfo: await Promise.all(indexList.map((index) => remote.getIndexInfo(index.name))),
                    autoIncrement: state.tableName === null ? false : await remote.hasTableAutoincrementColumn(state.tableName),
                    records,
                })
            } else {
                const numRecords = (await remote.query(`SELECT COUNT(*) as count FROM pragma_${state.pragma.toLowerCase()}`, [], "r"))[0]!.count
                if (typeof numRecords !== "bigint") { throw new Error(numRecords + "") }
                await get().setPaging({ numRecords })
                state = get() // state.paging will be updated
                if (state.tableName === undefined) { return }
                useTableStore.setState({
                    tableInfo: await remote.getTableInfo(`pragma_${state.pragma.toLowerCase()}`),
                    autoIncrement: false,
                    records: (await remote.query(`SELECT * FROM pragma_${state.pragma.toLowerCase()} LIMIT ? OFFSET ?`, [state.paging.pageSize, state.paging.visibleAreaTop], "r")) ?? [],
                })
            }
        },
        reloadVisibleArea: async () => {
            let state = get()
            if (state.tableName === undefined) { return }
            const { wr, type } = state.tableList.find(({ name }) => name === state.tableName) ?? {}
            if (wr === undefined || type === undefined) { return }
            // `AS rowid` is required for tables with a primary key because rowid is an alias of the primary key in that case.
            if (state.viewerStatement === "SELECT") {
                useTableStore.setState({
                    records: await remote.query(`SELECT ${(wr || type !== "table") ? "" : "rowid AS rowid, "}* FROM ${escapeSQLIdentifier(state.tableName)} ${state.viewerConstraints} LIMIT ? OFFSET ?`, [state.paging.pageSize, state.paging.visibleAreaTop], "r"),
                })
            } else {
                useTableStore.setState({
                    records: (await remote.query(`SELECT * FROM pragma_${state.pragma.toLowerCase()} LIMIT ? OFFSET ?`, [state.paging.pageSize, state.paging.visibleAreaTop], "r")) ?? [],
                })
            }
        },
    }
})

const App = () => {
    const state = useMainStore()

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
                            if (editorState.row === Number(paging.pageSize) - 1) {
                                await setPaging({ visibleAreaTop: paging.visibleAreaTop + 1n }, true)
                                editorState.update(editorState.tableName, editorState.column, Number(paging.pageSize) - 1)
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
                                editorState.clearInputs()
                            } else {
                                editorState.update(editorState.tableName, editorState.column, editorState.row)
                            }
                            break
                        case "ArrowUp":
                            if (!ev.ctrlKey && !ev.shiftKey && !ev.altKey && singleClick) { moveSelectionUp() }
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
                        case "Escape": editorState.clearInputs(); break
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
            }
        }, 1000)
        return () => { clearInterval(timer) }
    }, [])

    return <>
        <LoadingIndicator />
        <h2 className="first flex">
            <div className="whitespace-pre">
                <Select value={state.viewerStatement} onChange={(value) => { state.setViewerQuery({ viewerStatement: value }) }} options={{ SELECT: {}, PRAGMA: {} }} className="primary" />
                {state.viewerStatement === "SELECT" && <> * FROM
                    {" "}
                    {state.tableName === undefined ? <>No tables</> : <Select value={state.tableName} onChange={(value) => { state.setViewerQuery({ tableName: value }) }} options={Object.fromEntries(state.tableList.map(({ name: tableName }) => [tableName, {}] as const).sort((a, b) => a[0].localeCompare(b[0])))} className="primary" />}
                    {" "}
                </>}
            </div>
            <div className="flex-1">
                {state.viewerStatement === "SELECT" && <input value={state.viewerConstraints} onBlur={(ev) => { state.setViewerQuery({ viewerConstraints: ev.currentTarget.value }) }} placeholder={"WHERE <column> = <value> ORDER BY <column> ..."} autocomplete="off" className="w-full" />}
                {state.viewerStatement === "PRAGMA" && <Select value={state.pragma} onChange={(value) => state.setViewerQuery({ pragma: value })} options={Object.fromEntries(state.pragmaList.map((k) => [k, {}]))} />}
            </div>
        </h2>
        <div className="relative w-max max-w-full">
            <Table tableName={state.tableName} />
        </div>
        {state.errorMessage && <p className="text-white" style={{ background: "rgb(14, 72, 117)", padding: "10px" }}>
            <pre>{state.errorMessage}</pre>
            <Button value="Close" className="primary" style={{ marginTop: "10px" }} onClick={() => useMainStore.setState({ errorMessage: "" })} />
        </p>}
        <editor.Editor tableList={state.tableList} />
    </>
}

(async () => {
    await remote.downloadState()
    const tableList = await remote.getTableList()
    const tableName = tableList[0]?.name
    editor.useEditorStore.setState({ tableName })
    useMainStore.setState({
        tableList,
        pragmaList: (await remote.query("PRAGMA pragma_list", [], "r")).map(({ name }) => name as string),
    })
    useMainStore.getState().setViewerQuery({ tableName })
    render(<App />, document.body)
})().catch((err) => {
    console.error(err)
    document.write(err)
    document.write(useMainStore.getState().errorMessage)
})
