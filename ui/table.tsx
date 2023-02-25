import { useRef, useMemo, useLayoutEffect, useCallback, useState, useEffect } from "preact/hooks"
import * as remote from "./remote"
import { useEditorStore } from "./editor"
import produce from "immer"
import { scrollbarWidth, ScrollbarY } from "./scrollbar"
import { persistentRef, Tooltip } from "./components"
import { BigintMath, createStore } from "./util"
import deepEqual from "fast-deep-equal"
import type { JSXInternal } from "preact/src/jsx"

/** Build the WHERE clause from the state of the find widget */
const buildFindWidgetQuery = (tableInfo: remote.TableInfo) => {
    const { findWidget, isFindWidgetVisible } = useTableStore.getState()
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

export const useTableStore = createStore("useTableStore", {
    /** True if a change to the database by another process is detected but the table is not reloaded yet. */
    reloadRequired: false,
    /** True while the "Commit changes?" dialog is visible. */
    isConfirmDialogVisible: false as false | ((value: "commit" | "discard changes" | "cancel") => void),
    paging: {
        /** scrollTop */
        visibleAreaTop: 0n,
        /** the number of records in the table */
        numRecords: 0n,
        /** the number of rows visible at a time */
        visibleAreaSize: 20n,
    },
    /** error messages */
    errorMessage: "",
    /** a custom query `customViewerQuery` instead of "SELECT * FROM tableName" is used for the viewer if true */
    useCustomViewerQuery: false,
    /** a custom query for the viewer */
    customViewerQuery: "",
    /** the state of the find widget */
    findWidget: {
        value: "",
        caseSensitive: false,
        wholeWord: false,
        regex: false,
    },
    /** whether the find widget is visible */
    isFindWidgetVisible: false,
    /** The viewer reloads the table when the database is changed by another process when true. */
    autoReload: true,
    _rerender: {} as Record<string, never>,
    /** the list of tables in the database */
    tableList: [] as remote.TableListItem[],
    /** the name of the active table */
    tableName: undefined as string | undefined,
    /** error messages of the viewer */
    invalidQuery: null as string | null,
    /** the list of columns in the table */
    tableInfo: [] as remote.TableInfo,
    indexList: [] as remote.IndexList,
    indexInfo: [] as remote.IndexInfo[],
    indexSchema: [] as (string | null)[],
    tableSchema: null as string | null,
    /** Represents whether the active table has an AUTOINCREMENT column. */
    autoIncrement: false,
    /** the records in the visible area */
    records: [] as readonly { readonly [key in string]: Readonly<remote.SQLite3Value> }[],
    /** the state of the textbox shown over a cell */
    input: null as { readonly draftValue: JSXInternal.Element, readonly textarea: HTMLTextAreaElement | null } | null,
}, (set, get) => {
    /** Queries the visible area of the database table. */
    const reloadTable = async (reloadSchema: boolean, reloadRecordCount: boolean) => {
        try {
            let state = get()

            /** The table name, or a subquery if using a custom query. */
            let subquery: string
            /** True if `SELECT * FROM subquery` has row ids. */
            let hasRowId: boolean
            /** The list of columns of `SELECT * FROM subquery`. */
            let tableInfo: remote.TableInfo
            if (!state.useCustomViewerQuery) {
                if (state.tableName === undefined) { set({ invalidQuery: "" }); return }
                subquery = escapeSQLIdentifier(state.tableName)

                const tableListItem = state.tableList.find(({ name }) => name === state.tableName!)
                if (tableListItem === undefined) { set({ invalidQuery: "" }); return }
                hasRowId = tableListItem.type === "table" && !tableListItem.wr
                tableInfo = !reloadSchema ? get().tableInfo : await remote.getTableInfo(state.tableName, { withoutLogging: true })
            } else {
                if (state.customViewerQuery.trim() === "") { set({ invalidQuery: "" }); return }
                subquery = `(${state.customViewerQuery})`
                hasRowId = false
                tableInfo = !reloadSchema ? get().tableInfo : (await remote.query(`SELECT * FROM ${subquery} LIMIT 0`, [], "r", { withoutLogging: true })).columns.map((column, i): remote.TableInfo[number] => ({ name: column, cid: BigInt(i), dflt_value: null, notnull: 0n, pk: 0n, type: "" }))
            }

            // Filter records by the state of the find widget
            const { findWidgetQuery, findWidgetParams } = buildFindWidgetQuery(tableInfo)
            if (reloadRecordCount) {
                const numRecords = (await remote.query(`SELECT COUNT(*) as count FROM ${subquery}${findWidgetQuery}`, findWidgetParams, "r", { withoutLogging: true })).records[0]!.count
                if (typeof numRecords !== "bigint") { throw new Error(numRecords + "") }
                await setPaging({ numRecords }, undefined, true)
                state = get() // state.paging will be updated
            }

            // Query the database
            const records = (await remote.query(`SELECT${hasRowId ? " rowid AS rowid," : ""} * FROM ${subquery}${findWidgetQuery} LIMIT ? OFFSET ?`, [...findWidgetParams, state.paging.visibleAreaSize, state.paging.visibleAreaTop], "r", { withoutLogging: true })).records

            // Update the store
            if (!reloadSchema) {
                set({ records })
            } else {
                if (!state.useCustomViewerQuery) {
                    const indexList = await remote.getIndexList(state.tableName!, { withoutLogging: true })
                    const autoIncrement = await remote.hasTableAutoincrementColumn(state.tableName!, { withoutLogging: true })
                    const tableSchema = await remote.getTableSchema(state.tableName!, { withoutLogging: true })
                    set({
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
                    set({
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
            set({ invalidQuery: typeof err === "object" && err !== null && "message" in err ? err.message + "" : err + "" })
            throw err
        }
    }
    const setViewerQuery = async (opts: {
        useCustomViewerQuery?: boolean
        customViewerQuery?: string
        tableName?: string
    }) => {
        set(opts)
        await remote.setState("tableName", get().tableName)
        await reloadTable(true, true)
        if (opts.useCustomViewerQuery !== undefined || opts.customViewerQuery !== undefined || opts.tableName !== undefined) {
            await useEditorStore.getState().switchTable(opts.useCustomViewerQuery ? undefined : get().tableName)
        }
    }
    const listUniqueConstraints = () => {
        const uniqueConstraints: { primary: boolean, columns: string[] }[] = []
        const { tableInfo, indexList, indexInfo } = get()
        for (const column of tableInfo) {
            if (column.pk) {
                uniqueConstraints.push({ primary: true, columns: [column.name] })
            }
        }
        for (const [i, index] of indexList.entries()) {
            if (index.partial) { continue }
            if (!index.unique) { continue }
            uniqueConstraints.push({ primary: index.origin === "pk", columns: indexInfo[i]!.map(({ name }) => name) })
        }
        return uniqueConstraints
    }
    const setPaging = async (opts: { visibleAreaTop?: bigint, numRecords?: bigint, visibleAreaSize?: bigint }, preserveEditorState?: true, withoutTableReloading?: true) => {
        const paging = { ...get().paging, ...opts }
        paging.visibleAreaSize = BigintMath.max(1n, BigintMath.min(200n, paging.visibleAreaSize))
        paging.visibleAreaTop = BigintMath.max(0n, BigintMath.min(paging.numRecords - paging.visibleAreaSize, paging.visibleAreaTop))
        if (deepEqual(get().paging, paging)) { return }
        if (!await useEditorStore.getState().commitUpdate()) { return }

        await remote.setState("visibleAreaSize", Number(paging.visibleAreaSize))
        if (!preserveEditorState) { await useEditorStore.getState().discardChanges() }
        paging.visibleAreaTop = BigintMath.max(0n, BigintMath.min(paging.numRecords - paging.visibleAreaSize, paging.visibleAreaTop))
        set({ paging })
        if (!withoutTableReloading) {
            await reloadTable(false, false)
        }
    }
    return {
        setViewerQuery,
        listUniqueConstraints,
        reloadTable,
        setPaging,
        /** Displays `confirm("Commit changes?")` using a `<dialog>`. */
        confirm: async () => new Promise<"commit" | "discard changes" | "cancel">((resolve) => {
            set({
                isConfirmDialogVisible: (value) => {
                    set({ isConfirmDialogVisible: false })
                    resolve(value)
                }
            })
        }),
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
        rerender: () => set({ _rerender: {} }),
        addErrorMessage: (value: string) => set((s) => ({ errorMessage: s.errorMessage + value + "\n" })),
        /** Enumerates the column tuples that uniquely select the record. */
        getRecordSelectors: (record: Record<string, remote.SQLite3Value>): string[][] => {
            const constraintChoices = ("rowid" in record ? [["rowid"]] : [])
                .concat(listUniqueConstraints().sort((a, b) => +b.primary - +a.primary)
                    .map(({ columns }) => columns)
                    .filter((columns) => columns.every((column) => record[column] !== null)))
            return [...new Set(constraintChoices.map((columns) => JSON.stringify(columns.sort((a, b) => a.localeCompare(b)))))].map((columns) => JSON.parse(columns))  // Remove duplicates
        },
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

export const Table = ({ tableName }: { tableName: string | undefined }) => {
    const visibleAreaTop = useTableStore((s) => Number(s.paging.visibleAreaTop))
    const pageSize = useTableStore((s) => Number(s.paging.visibleAreaSize))
    const numRecords = useTableStore((s) => Number(s.paging.numRecords))
    const invalidQuery = useTableStore((s) => s.invalidQuery)
    const tableInfo = useTableStore((s) => s.tableInfo)
    const autoIncrement = useTableStore((s) => s.autoIncrement)
    const records = useTableStore((s) => s.records)
    const input = useTableStore((s) => s.input)
    const setPaging = useTableStore((s) => s.setPaging)

    const alterTable = useEditorStore((s) => s.alterTable)
    const selectedRow = useEditorStore((s) => s.statement === "DELETE" ? s.row : null)
    const selectedDataRow = useEditorStore((s) => s.statement === "UPDATE" ? s.row : null)
    const selectedDataColumn = useEditorStore((s) => s.statement === "UPDATE" ? s.column : null)
    const commitUpdate = useEditorStore((s) => s.commitUpdate)
    const isDirty = useEditorStore((s) => s.isDirty)

    const columnWidthCaches = persistentRef<Record<string, (number | undefined | null)[]>>("columnWidths_v3", {})
    const getColumnWidths = () => {
        const result: number[] = []
        for (let i = 0; i < tableInfo.length; i++) {
            result.push(columnWidthCaches.current[tableName ?? ""]?.[i] ?? (tableInfo.length === 1 ? 250 : 150))
        }
        return result
    }
    const tableRef = useRef<HTMLTableElement>(null)

    const scrollbarRef = useRef<ScrollbarY>(null)
    useEffect(() => {
        if (!tableRef.current) { return }
        const el = tableRef.current
        const onWheel = (ev: WheelEvent) => {
            if (ev.deltaY === 0) { return }  // Scroll horizontally
            ev.preventDefault()
            scrollbarRef.current!.wheel(ev.deltaY / 30)
        }
        el.addEventListener("wheel", onWheel, { passive: false })
        return () => { el.removeEventListener("wheel", onWheel as any, { passive: false } as any) }
    }, [tableRef.current])

    const isFindWidgetVisible = useTableStore((s) => s.isFindWidgetVisible)
    const tableType = useTableStore((s) => s.tableList.find(({ name }) => name === s.tableName)?.type)

    if (invalidQuery !== null) {
        return <span class="text-red-700">{invalidQuery}</span>
    }

    return <>
        <div class="max-w-full overflow-x-auto w-max">
            {/* Table */}
            <table ref={tableRef} class="viewer w-max border-collapse table-fixed bg-white" style={{ paddingRight: scrollbarWidth, boxShadow: "0 0 0px 2px #000000ad" }} data-testid="viewer-table">
                {/* Table Header */}
                <thead class="text-black bg-[var(--gutter-color)]" style={{ outline: "rgb(181, 181, 181) 1px solid" }}>
                    <tr>
                        <th class="font-normal select-none pt-[3px] pb-[3px] pl-[1em] pr-[1em]"></th>
                        {tableInfo.map(({ name, notnull, pk, type, dflt_value }, i) => <th
                            style={{ width: getColumnWidths()[i]! }}
                            class={"font-normal select-none " + (tableName !== undefined && tableType === "table" ? "clickable" : "")}
                            title={tableType === "table" ? "" : `A ${tableType} cannot be altered.`}
                            onMouseMove={(ev) => {
                                const rect = ev.currentTarget.getBoundingClientRect()
                                if (rect.right - ev.clientX < 20) {
                                    ev.currentTarget.classList.add("ew-resize")
                                } else {
                                    ev.currentTarget.classList.remove("ew-resize")
                                }
                            }}
                            onMouseDown={(ev) => {
                                if (isDirty()) {
                                    commitUpdate().catch(console.error)
                                    return
                                }

                                const th = ev.currentTarget
                                const rect = th.getBoundingClientRect()
                                if (rect.right - ev.clientX < 20) {
                                    const mouseMove = (ev: MouseEvent) => {
                                        columnWidthCaches.current = produce(columnWidthCaches.current, (d) => {
                                            if (!Array.isArray(d[tableName ?? ""])) {
                                                d[tableName ?? ""] = []
                                            }
                                            d[tableName ?? ""]![i] = Math.max(50, ev.clientX - rect.left)
                                        })
                                        th.style.width = columnWidthCaches.current[tableName ?? ""]![i]! + "px"
                                        for (const td of tableRef.current?.querySelectorAll<HTMLElement>(`td:nth-child(${i + 2})`) ?? []) {
                                            td.style.maxWidth = columnWidthCaches.current[tableName ?? ""]![i]! + "px"
                                        }
                                    }
                                    document.body.classList.add("ew-resize")
                                    window.addEventListener("mousemove", mouseMove)
                                    window.addEventListener("mouseup", () => {
                                        window.removeEventListener("mousemove", mouseMove)
                                        document.body.classList.remove("ew-resize")
                                    }, { once: true })
                                } else if (tableName !== undefined && tableType === "table") { // center
                                    alterTable(tableName, name).catch(console.error)
                                }
                            }}
                            onMouseLeave={(ev) => {
                                ev.currentTarget.classList.remove("ew-resize")
                            }}>
                            <code class="inline-block [word-break:break-word] [color:inherit] [font-family:inherit] [font-size:inherit]">
                                {name}
                                <span class="italic opacity-70">{`${type ? (" " + type) : ""}${pk ? (autoIncrement ? " PRIMARY KEY AUTOINCREMENT" : " PRIMARY KEY") : ""}${notnull ? " NOT NULL" : ""}${dflt_value !== null ? ` DEFAULT ${dflt_value}` : ""}`}</span>
                            </code>
                        </th>)}
                    </tr>
                </thead>

                {/* Table Body */}
                <tbody>
                    {/* Find Widget */}
                    {isFindWidgetVisible && <tr>
                        <td class="pl-[10px] pr-[10px] bg-[var(--gutter-color)]"></td>
                        <td class="relative text-right" colSpan={tableInfo.length} style={{ maxWidth: getColumnWidths().reduce((a, b) => a + b, 0) }}>
                            <FindWidget />
                        </td>
                    </tr>}

                    {/* Placeholder that is displayed when the table has no rows */}
                    {records.length === 0 && <tr>
                        <td class="overflow-hidden no-hover inline-block cursor-default h-[1.2em] pl-[10px] pr-[10px]" style={{ borderRight: "1px solid var(--td-border-color)" }}></td>
                    </tr>}

                    {/* Rows */}
                    {records.map((record, row) => <TableRow selected={selectedRow === row} key={row} row={row} selectedColumn={selectedDataColumn} input={selectedDataRow === row ? input : null} tableName={tableName} tableInfo={tableInfo} record={record} columnWidths={getColumnWidths()} rowNumber={BigInt(visibleAreaTop + row) + 1n} />)}
                </tbody>
            </table>
        </div>

        {/* Vertical Scroll Bar */}
        {// @ts-ignore
            <scrollbar-y
                ref={scrollbarRef}
                min={0}
                max={numRecords}
                size={pageSize}
                value={visibleAreaTop}
                class="h-full right-0 top-0 absolute"
                style={{ width: scrollbarWidth }}
                onChange={() => { setPaging({ visibleAreaTop: BigInt(Math.round(scrollbarRef.current!.value)) }).catch(console.error) }} />}
    </>
}

/** Renders a search widget with toggle buttons for case sensitivity, whole word, and regular expression. */
const FindWidget = () => {
    const { value, caseSensitive, wholeWord, regex } = useTableStore((s) => s.findWidget)
    const setFindWidgetState = useTableStore((s) => s.setFindWidgetState)
    const ref = useRef<HTMLInputElement>(null)

    // Focus the input box on mount
    useEffect(() => {
        ref.current!.focus()
        ref.current!.select()
    }, [])

    return <div class="inline-block pl-1 pt-1 bg-gray-200 shadow-md whitespace-nowrap sticky right-3">
        {/* Text input */}
        <input id="findWidget" ref={ref} class="mr-1" placeholder="Find" value={value} onChange={(ev) => setFindWidgetState({ value: ev.currentTarget.value })} />

        {/* Match Case button */}
        <Tooltip content="Match Case" ><span class="[font-size:130%] align-middle text-gray-600 hover:bg-gray-300 select-none p-[2px] [border-radius:1px] inline-block cursor-pointer" style={caseSensitive ? { background: "rgba(66, 159, 202, 0.384)", color: "black" } : {}} onClick={() => setFindWidgetState({ caseSensitive: !caseSensitive })}>
            <svg class="w-[1em] h-[1em]"><use xlinkHref="#case-sensitive" /></svg>
        </span></Tooltip>

        {/* Match Whole Word button */}
        <Tooltip content="Match Whole Word"><span class="[font-size:130%] align-middle text-gray-600 hover:bg-gray-300 select-none p-[2px] [border-radius:1px] inline-block cursor-pointer" style={wholeWord ? { background: "rgba(66, 159, 202, 0.384)", color: "black" } : {}} onClick={() => setFindWidgetState({ wholeWord: !wholeWord })}>
            <svg class="w-[1em] h-[1em]"><use xlinkHref="#whole-word" /></svg>
        </span></Tooltip>

        {/* Use Regular Expression button */}
        <Tooltip content="Use Regular Expression"><span class="[font-size:130%] align-middle text-gray-600 hover:bg-gray-300 select-none p-[2px] [border-radius:1px] inline-block cursor-pointer" style={regex ? { background: "rgba(66, 159, 202, 0.384)", color: "black" } : {}} onClick={() => setFindWidgetState({ regex: !regex })}>
            <svg class="w-[1em] h-[1em]"><use xlinkHref="#regex" /></svg>
        </span></Tooltip>
    </div>
}

const TableRow = (props: { selected: boolean, readonly selectedColumn: string | null, input: { readonly draftValue: JSXInternal.Element, readonly textarea: HTMLTextAreaElement | null } | null, tableName: string | undefined, tableInfo: remote.TableInfo, record: { readonly [key in string]: Readonly<remote.SQLite3Value> }, rowNumber: bigint, row: number, columnWidths: readonly number[] }) => {
    if (props.rowNumber <= 0) {
        throw new Error(props.rowNumber + "")
    }
    const delete_ = useEditorStore((s) => s.delete_)
    const update = useEditorStore((s) => s.update)
    const commitUpdate = useEditorStore((s) => s.commitUpdate)

    const [cursorVisibility, setCursorVisibility] = useState(true)
    const onFocusOrMount = useCallback(() => { setCursorVisibility(true) }, [])
    const onBlurOrUnmount = useCallback(() => { setCursorVisibility(false) }, [])

    return useMemo(() => <tr class={props.selected ? "editing" : ""}>
        {/* Row number */}
        <td
            class={"pl-[10px] pr-[10px] bg-[var(--gutter-color)] overflow-hidden sticky left-0 whitespace-nowrap text-right text-black select-none " + (props.tableName !== undefined ? "clickable" : "")}
            style={{ borderRight: "1px solid var(--td-border-color)" }}
            onMouseDown={() => {
                (async () => {
                    if (!await commitUpdate()) { return }
                    if (props.tableName === undefined) { return }
                    await delete_(props.tableName, props.record, props.row)
                })().catch(console.error)
            }}
            data-testid={`row number ${props.rowNumber}`}>{props.rowNumber}</td>

        {/* Cells */}
        {props.tableInfo.map(({ name }, i) => {
            const value = props.record[name] as remote.SQLite3Value
            const input = props.selectedColumn === name ? props.input : undefined
            return <td
                class={"pl-[10px] pr-[10px] overflow-hidden " + (props.tableName !== undefined ? "clickable" : "") + " " + (input ? "editing" : "")}
                style={{ borderRight: "1px solid var(--td-border-color)", maxWidth: props.columnWidths[i], borderBottom: "1px solid var(--td-border-color)" }}
                onMouseDown={() => {
                    (async () => {
                        const editorState = useEditorStore.getState()
                        if (editorState.statement === "UPDATE" && editorState.row === props.row && editorState.column === name) { return }
                        if (!await commitUpdate()) { return }
                        if (props.tableName === undefined) { return }
                        update(props.tableName, name, props.row)
                    })().catch(console.error)
                }}
                data-testid={`cell ${props.rowNumber - 1n}, ${i}`}>
                <pre class={"overflow-hidden text-ellipsis whitespace-nowrap max-w-[50em] [font-size:inherit] " + (input?.textarea && cursorVisibility ? "cursor-line" : "")}>
                    <span class="select-none">{input?.draftValue ?? renderValue(value)}</span>
                    {input?.textarea && <MountInput element={input.textarea} onFocusOrMount={onFocusOrMount} onBlurOrUnmount={onBlurOrUnmount} />}
                </pre>
            </td>
        })}
    </tr>, [props.selected, props.selectedColumn, props.input?.draftValue, props.input?.textarea, props.tableName, props.tableInfo, props.record, props.rowNumber, cursorVisibility])  // excluded: props.columnWidth
}

/** Renders `<span>{props.element}</span>` and focuses the `props.element`. `props.onFocusOrMount` and `props.onBlurOrUnmount` will be called when the `props.element` is focused/unfocused or the value of `props.element` is changed. */
const MountInput = (props: { element: HTMLTextAreaElement, onFocusOrMount: () => void, onBlurOrUnmount: () => void }) => {
    const ref = useRef<HTMLSpanElement>(null)
    useEffect(() => {
        ref.current?.append(props.element)
        props.onFocusOrMount()
        props.element.addEventListener("focus", () => { props.onFocusOrMount() })
        props.element.addEventListener("blur", () => { props.onBlurOrUnmount() })
        props.element.select()
        props.element.focus()
        return () => {
            props.onBlurOrUnmount()
            if (ref.current?.contains(props.element)) { ref.current.removeChild(props.element) }
        }
    }, [props.element])
    return <span ref={ref}></span>
}

/** Not verified for safety. */
export const unsafeEscapeValue = (value: remote.SQLite3Value) => {
    if (value instanceof Uint8Array) {
        return `x'${blob2hex(value, undefined)}'`
    } else if (value === null) {
        return "NULL"
    } else if (typeof value === "string") {
        return `'${value.replaceAll("'", "''").replaceAll("\r", "\\r").replaceAll("\n", "\\n")}'`
    } else if (typeof value === "number") {
        return /^[+\-]?\d+$/.test("" + value) ? "" + value + ".0" : "" + value
    } else {
        return "" + value
    }
}

export const renderValue = (value: remote.SQLite3Value): JSXInternal.Element => {
    if (value instanceof Uint8Array) {  // BLOB
        return <span class="[color:var(--data-null)]">{`x'${blob2hex(value, 8)}'`}</span>
    } else if (value === null) {  // NULL
        return <span class="[color:var(--data-null)]">NULL</span>
    } else if (typeof value === "string") {  // TEXT
        return <span class="[color:var(--data-string)]">{value.replaceAll("\\", "\\\\").replaceAll("\t", "\\t").replaceAll("\r", "\\r").replaceAll("\n", "\\n") || /* nbsp */"\u00a0"}</span>
    } else if (typeof value === "number") {  // REAL
        return <span class="[color:var(--data-number)]">{/^[+\-]?\d+$/.test("" + value) ? "" + value + ".0" : "" + value}</span>
    } else {  // INTEGER
        return <span class="[color:var(--data-number)]">{"" + value}</span>
    }
}

/** https://stackoverflow.com/a/6701665/10710682, https://stackoverflow.com/a/51574648/10710682 */
export const escapeSQLIdentifier = (ident: string) => {
    if (ident.includes("\x00")) { throw new Error("Invalid identifier") }
    return ident.includes('"') || /[^A-Za-z0-9_\$]/.test(ident) ? `"${ident.replaceAll('"', '""')}"` : ident
}

export const blob2hex = (blob: Uint8Array, maxLength?: number) =>
    Array.from(blob.slice(0, maxLength), (x) => x.toString(16).padStart(2, "0")).join("") + (maxLength !== undefined && blob.length > maxLength ? "..." : "")

export const type2color = (type: "number" | "bigint" | "string" | "null" | "blob" | "default") => {
    if (type === "number" || type === "bigint") {
        return "var(--data-number)"
    } else if (type === "string") {
        return "var(--data-string)"
    } else {
        return "var(--data-null)"
    }
}
