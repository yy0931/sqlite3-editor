import { useRef, useMemo, useCallback, useState, useEffect } from "preact/hooks"
import * as remote from "./remote"
import { useEditorStore } from "./editor"
import produce from "immer"
import { scrollbarWidth, ScrollbarY } from "./scrollbar"
import { flash, persistentRef, renderContextmenu, Tooltip } from "./components"
import { BigintMath, createStore, querySelectorWithRetry } from "./util"
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
    /** the columns to be queried, sorted by tableInfo. */
    visibleColumns: [] as string[],
    /** error messages of the viewer */
    invalidQuery: null as string | null,
    /** the list of columns in the table */
    tableInfo: [] as remote.TableInfo,
    foreignKeyList: [] as remote.ForeignKeyList,
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
            const records = (await remote.query(`SELECT${hasRowId ? " rowid AS rowid," : ""} ${getVisibleColumnsSQL()} FROM ${subquery}${findWidgetQuery}${hasRowId ? " ORDER BY rowid" : ""} LIMIT ? OFFSET ?`, [...findWidgetParams, state.paging.visibleAreaSize, state.paging.visibleAreaTop], "r", { withoutLogging: true })).records

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
                        foreignKeyList: await remote.getForeignKeyList(state.tableName!, { withoutLogging: true }),
                        visibleColumns: tableInfo.map(({ name }) => name),
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
                        foreignKeyList: [],
                        visibleColumns: tableInfo.map(({ name }) => name),
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
        paging.visibleAreaTop = BigintMath.max(0n, BigintMath.min(paging.numRecords - paging.visibleAreaSize + 1n, paging.visibleAreaTop))
        if (deepEqual(get().paging, paging)) { return }
        if (!await useEditorStore.getState().beforeUnmount()) { return }

        if (!preserveEditorState) { await useEditorStore.getState().discardChanges() }
        paging.visibleAreaTop = BigintMath.max(0n, BigintMath.min(paging.numRecords - paging.visibleAreaSize + 1n, paging.visibleAreaTop))

        // Update paging before querying the database. Otherwise the scrollbar will vibrate.
        set({ paging })

        await remote.setState("visibleAreaSize", Number(paging.visibleAreaSize))
        if (!withoutTableReloading) {
            await reloadTable(false, false)
        }
    }
    /** Render visibleColumns to be embedded in SQL queries. */
    const getVisibleColumnsSQL = () => {
        const { tableInfo, visibleColumns } = get()
        return tableInfo.length === visibleColumns.length ? "*" : visibleColumns.map(escapeSQLIdentifier).join(", ")
    }
    return {
        setViewerQuery,
        listUniqueConstraints,
        reloadTable,
        setPaging,
        getVisibleColumnsSQL,
        setVisibleColumns: async (columns: string[]) => {
            if (columns.length === 0) { throw new Error() }
            const { tableInfo } = get()
            columns = [...new Set(columns)]  // remove duplicates
            if (columns.some((column) => !tableInfo.find((v) => v.name === column))) { throw new Error() }
            set({ visibleColumns: columns.sort((a, b) => tableInfo.findIndex((v) => v.name === a) - tableInfo.findIndex((v) => v.name === b)) })
            await reloadTable(false, false)
        },
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

export const Table = () => {
    const tableName = useTableStore((s) => s.tableName)
    const visibleAreaTop = useTableStore((s) => Number(s.paging.visibleAreaTop))
    const visibleAreaSize = useTableStore((s) => Number(s.paging.visibleAreaSize))
    const numRecords = useTableStore((s) => Number(s.paging.numRecords))
    const invalidQuery = useTableStore((s) => s.invalidQuery)
    const tableInfo = useTableStore((s) => s.tableInfo)
    const autoIncrement = useTableStore((s) => s.autoIncrement)
    const records = useTableStore((s) => s.records)
    const input = useTableStore((s) => s.input)
    const setPaging = useTableStore((s) => s.setPaging)
    const visibleColumns = useTableStore((s) => s.visibleColumns)
    const setVisibleColumns = useTableStore((s) => s.setVisibleColumns)

    const alterTable = useEditorStore((s) => s.alterTable)
    const selectedRow = useEditorStore((s) => s.statement === "DELETE" ? s.row : null)
    const selectedDataRow = useEditorStore((s) => s.statement === "UPDATE" ? s.row : null)
    const selectedDataColumn = useEditorStore((s) => s.statement === "UPDATE" ? s.column : null)
    const beforeUnmount = useEditorStore((s) => s.beforeUnmount)
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
    const foreignKeyList = useTableStore((s) => s.foreignKeyList)

    if (invalidQuery !== null) {
        return <span class="text-red-700">{invalidQuery}</span>
    }

    return <>
        <div class="max-w-full overflow-x-auto w-max">
            {/* Table */}
            <table ref={tableRef} class="viewer w-max border-collapse table-fixed bg-white [box-shadow:0_0_0px_2px_#000000ad]" style={{ paddingRight: scrollbarWidth }} data-testid="viewer-table">
                {/* Table Header */}
                <thead class="text-black bg-[var(--gutter-color)] [outline:rgb(181,181,181)_1px_solid]">
                    <tr>
                        <th class="font-normal select-none pt-[3px] pb-[3px] pl-[1em] pr-[1em]"></th>
                        {tableInfo.filter((v) => visibleColumns.includes(v.name)).map(({ name, notnull, pk, type, dflt_value }, i) => <th
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
                                if (ev.button === 2) { return } // right click
                                if (isDirty()) {
                                    beforeUnmount().catch(console.error)
                                    return  // Always return because the user needs to stop dragging when a dialog is displayed.
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
                            }}
                            onContextMenu={(ev) => {
                                renderContextmenu(ev, <>
                                    {tableName !== undefined && tableType === "table" && <button onClick={async () => {
                                        if (!await beforeUnmount()) { return }
                                        await alterTable(tableName, name, "RENAME COLUMN")
                                        flash(document.querySelector("#editor")!)
                                    }}>Rename…</button>}
                                    {tableName !== undefined && tableType === "table" && <button onClick={async () => {
                                        if (!await beforeUnmount()) { return }
                                        await alterTable(tableName, name, "DROP COLUMN")
                                        flash(document.querySelector("#editor")!)
                                    }}>Delete…</button>}
                                    {tableName !== undefined && tableType === "table" && <button onClick={async () => {
                                        if (!await beforeUnmount()) { return }
                                        await alterTable(tableName, name, "ADD COLUMN")
                                        flash(document.querySelector("#editor")!)
                                    }}>Add Column…</button>}
                                    <hr />
                                    <button disabled={visibleColumns.length === 1} onClick={async () => {
                                        await setVisibleColumns(visibleColumns.filter((v) => v !== name))
                                    }}>Hide</button>
                                </>)
                            }}>
                            <code class="inline-block [word-break:break-word] [color:inherit] [font-family:inherit] [font-size:inherit]">
                                {name}
                                <span class="italic opacity-70">
                                    {!!type && <span> {type}</span>}
                                    {!!notnull && <span> NOT NULL</span>}
                                    {dflt_value !== null && <span> DEFAULT {dflt_value}</span>}
                                    {!!pk && <span> {autoIncrement ? <>PRIMARY KEY AUTOINCREMENT</> : <>PRIMARY KEY</>}</span>}
                                    {foreignKeyList.filter(({ to }) => to === name)
                                        .map((foreignKey) => <span> REFERENCES {escapeSQLIdentifier(foreignKey.table)}({escapeSQLIdentifier(foreignKey.from)})</span>)}
                                </span>
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

                    {/* Rows */}
                    {records.map((record, row) => <TableRow selected={selectedRow === row} key={row} row={row} selectedColumn={selectedDataColumn} input={selectedDataRow === row ? input : null} record={record} columnWidths={getColumnWidths()} rowNumber={BigInt(visibleAreaTop + row) + 1n} />)}

                    {/* Margin */}
                    {/* NOTE: `visibleAreaSize - records.length` can be negative while resizing the table. */}
                    {Array(Math.max(0, visibleAreaSize - records.length)).fill(0).map((_, row) => <EmptyTableRow key={row} row={row} columnWidths={getColumnWidths()} rowNumber={BigInt(visibleAreaTop + records.length + row) + 1n} />)}
                </tbody>
            </table>
        </div>

        {/* Vertical Scroll Bar */}
        {// @ts-ignore
            <scrollbar-y
                ref={scrollbarRef}
                min={0}
                max={numRecords + 1}
                size={visibleAreaSize}
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
        <Tooltip content="Match Case" ><span class={"[font-size:130%] align-middle select-none p-[2px] [border-radius:1px] inline-block cursor-pointer " + (caseSensitive ? "bg-blue-400 bg-opacity-40 text-black" : "text-gray-600 hover:bg-gray-300")} onClick={() => setFindWidgetState({ caseSensitive: !caseSensitive })}>
            <svg class="w-[1em] h-[1em]"><use xlinkHref="#case-sensitive" /></svg>
        </span></Tooltip>

        {/* Match Whole Word button */}
        <Tooltip content="Match Whole Word"><span class={"[font-size:130%] align-middle select-none p-[2px] [border-radius:1px] inline-block cursor-pointer " + (wholeWord ? "bg-blue-400 bg-opacity-40 text-black" : "text-gray-600 hover:bg-gray-300")} onClick={() => setFindWidgetState({ wholeWord: !wholeWord })}>
            <svg class="w-[1em] h-[1em]"><use xlinkHref="#whole-word" /></svg>
        </span></Tooltip>

        {/* Use Regular Expression button */}
        <Tooltip content="Use Regular Expression"><span class={"[font-size:130%] align-middle select-none p-[2px] [border-radius:1px] inline-block cursor-pointer " + (regex ? "bg-blue-400 bg-opacity-40 text-black" : "text-gray-600 hover:bg-gray-300")} onClick={() => setFindWidgetState({ regex: !regex })}>
            <svg class="w-[1em] h-[1em]"><use xlinkHref="#regex" /></svg>
        </span></Tooltip>
    </div>
}

const TableRow = (props: { selected: boolean, readonly selectedColumn: string | null, input: { readonly draftValue: JSXInternal.Element, readonly textarea: HTMLTextAreaElement | null } | null, record: { readonly [key in string]: Readonly<remote.SQLite3Value> }, rowNumber: bigint, row: number, columnWidths: readonly number[] }) => {
    if (props.rowNumber <= 0) {
        throw new Error(props.rowNumber + "")
    }
    const tableName = useTableStore((s) => s.tableName)
    const tableInfo = useTableStore((s) => s.tableInfo)
    const visibleColumns = useTableStore((s) => s.visibleColumns)

    const delete_ = useEditorStore((s) => s.delete_)
    const update = useEditorStore((s) => s.update)
    const beforeUnmount = useEditorStore((s) => s.beforeUnmount)

    const [cursorVisibility, setCursorVisibility] = useState(true)
    const onFocusOrMount = useCallback(() => { setCursorVisibility(true) }, [])
    const onBlurOrUnmount = useCallback(() => { setCursorVisibility(false) }, [])

    return useMemo(() => <tr class={props.selected ? "editing" : ""}>
        {/* Row number */}
        <td
            class={"pl-[10px] pr-[10px] bg-[var(--gutter-color)] overflow-hidden sticky left-0 whitespace-nowrap text-right text-black select-none border-r-[1px] border-r-[var(--td-border-color)] " + (tableName !== undefined ? "clickable" : "")}
            onMouseDown={async (ev) => {
                if (ev.button === 2) { return } // context menu
                ev.preventDefault()
                if (tableName === undefined) { return }
                if (!await beforeUnmount()) { return }
                await delete_(tableName, props.record, props.row)
            }}
            onContextMenu={(ev) => {
                renderContextmenu(ev, <>
                    <button onClick={async () => {
                        if (tableName === undefined) { return }
                        if (!await beforeUnmount()) { return }
                        await delete_(tableName, props.record, props.row)
                    }}>Delete…</button>
                </>)
            }}
            data-testid={`row number ${props.rowNumber}`}>{props.rowNumber}</td>

        {/* Cells */}
        {tableInfo.filter((v) => visibleColumns.includes(v.name)).map(({ name }, i) => {
            const value = props.record[name] as remote.SQLite3Value
            const input = props.selectedColumn === name ? props.input : undefined
            const onMouseDown = async (ev: MouseEvent) => {
                if (ev.target instanceof HTMLTextAreaElement) { return }  // in-place input
                ev.preventDefault()
                if (ev.button === 2) { return }
                const editorState = useEditorStore.getState()
                if (editorState.statement === "UPDATE" && editorState.row === props.row && editorState.column === name) { return }
                if (tableName === undefined) { return }
                if (!await beforeUnmount()) { return }
                update(tableName, name, props.row)
            }
            return <td
                class={"pl-[10px] pr-[10px] overflow-hidden border-r-[1px] border-[var(--td-border-color)] border-b-[1px] border-b-[var(--td-border-color)] " + (tableName !== undefined ? "clickable" : "") + " " + (input ? "editing" : "")}
                style={{ maxWidth: props.columnWidths[i] }}
                onMouseDown={onMouseDown}
                onContextMenu={(ev) => {
                    if (input?.textarea && !input.textarea.classList.contains("single-click")) { return }  // if the in-place input is visible
                    ev.preventDefault()
                    renderContextmenu(ev, <>
                        <button onClick={onMouseDown}>Update</button>
                        <hr />
                        <button onClick={() => {
                            if (value instanceof Uint8Array) {  // BLOB
                                navigator.clipboard.writeText(blob2hex(value)).catch(console.error)
                            } else if (value === null) {  // NULL
                                navigator.clipboard.writeText("NULL").catch(console.error)
                            } else {
                                navigator.clipboard.writeText("" + value).catch(console.error)
                            }
                        }}>Copy Text</button>
                        <button onClick={() => navigator.clipboard.writeText(JSON.stringify(value))}>Copy JSON</button>
                        <button onClick={() => navigator.clipboard.writeText(unsafeEscapeValue(value))}>Copy SQL</button>
                    </>)
                }}
                data-testid={`cell ${props.rowNumber - 1n}, ${i}`}>
                <pre class={"overflow-hidden text-ellipsis whitespace-nowrap max-w-[50em] [font-size:inherit] " + (input?.textarea && cursorVisibility ? "cursor-line" : "")}>
                    <span class="select-none">{input?.draftValue ?? renderValue(value)}</span>
                    {input?.textarea && <MountInput element={input.textarea} onFocusOrMount={onFocusOrMount} onBlurOrUnmount={onBlurOrUnmount} />}
                </pre>
            </td>
        })}
    </tr>, [props.selected, props.selectedColumn, props.input?.draftValue, props.input?.textarea, tableName, tableInfo, visibleColumns, props.record, props.rowNumber, cursorVisibility])  // excluded: props.columnWidth
}

/** Renders an empty row that is shown at the bottom of the table. */
const EmptyTableRow = (props: { row: number, rowNumber: bigint, columnWidths: readonly number[] }) => {
    const tableName = useTableStore((s) => s.tableName)
    const tableInfo = useTableStore((s) => s.tableInfo)
    const visibleColumns = useTableStore((s) => s.visibleColumns)

    const insert = useEditorStore((s) => s.insert)
    const beforeUnmount = useEditorStore((s) => s.beforeUnmount)
    const statement = useEditorStore((s) => s.statement)

    const openInsertEditor = async () => {
        if (!tableName) { return }
        if (statement !== "INSERT") {
            if (!await beforeUnmount()) { return }
            await insert(tableName)
        }
    }

    return <tr>
        {/* Row number */}
        <td
            class={"pl-[10px] pr-[10px] bg-[var(--gutter-color)] overflow-hidden sticky left-0 whitespace-nowrap text-center text-black select-none border-r-[1px] border-r-[var(--td-border-color)] " + (tableName !== undefined && props.row === 0 ? "clickable" : "")}
            onMouseDown={(ev) => {
                ev.preventDefault()
                if (props.row !== 0) { return }
                (async () => {
                    await openInsertEditor()

                    // Find a textarea
                    // NOTE: On chrome querySelector() always returns an element, but on VSCode's webview rendering is slow and querySelector() always returns null for the first try.
                    const textarea = await querySelectorWithRetry<HTMLTextAreaElement>("#editor textarea")
                    if (!textarea) { return }

                    // Select the textarea
                    textarea.focus()
                    textarea.select()

                    // Play an animation
                    flash(textarea)
                })().catch(console.error)
            }}>{props.row === 0 && <svg class="inline w-[1em] h-[1em]"><use xlinkHref="#add" /></svg>}</td>

        {/* Cells */}
        {tableInfo.filter((v) => visibleColumns.includes(v.name)).map(({ }, i) => <td
            class={"pl-[10px] pr-[10px] overflow-hidden border-r-[1px] border-r-[var(--td-border-color)] border-b-[1px] border-b-[var(--td-border-color)] " + (tableName !== undefined ? "clickable" : "")}
            style={{ maxWidth: props.columnWidths[i] }}
            onMouseDown={(ev) => {
                ev.preventDefault();
                (async () => {
                    await openInsertEditor()

                    // NOTE: querySelector() always returns an element in the browser, but in VSCode's webview, rendering is slow and querySelector() always returns null if you don't wait.
                    const textarea = await querySelectorWithRetry<HTMLTextAreaElement>(`#insert-column${i + 1} textarea`)
                    if (!textarea) { return }

                    // Select the textarea
                    textarea.focus()
                    textarea.select()

                    // Play an animation
                    flash(textarea)
                })().catch(console.error)
            }}>
            <pre class="overflow-hidden text-ellipsis whitespace-nowrap max-w-[50em] [font-size:inherit] ">
                <span class="select-none">&nbsp;</span>
            </pre>
        </td>)}
    </tr>
}

/** Renders `<span>{props.element}</span>` and focuses the `props.element`. `props.onFocusOrMount` and `props.onBlurOrUnmount` will be called when the `props.element` is focused/unfocused or the value of `props.element` is changed. */
const MountInput = (props: { element: HTMLTextAreaElement, onFocusOrMount: () => void, onBlurOrUnmount: () => void }) => {
    const ref = useRef<HTMLSpanElement>(null)
    useEffect(() => {
        ref.current?.append(props.element)
        props.onFocusOrMount()
        props.element.addEventListener("focus", () => { props.onFocusOrMount() })
        props.element.addEventListener("blur", () => { props.onBlurOrUnmount() })
        props.element.focus()
        props.element.select()
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
        return <span class="text-[var(--data-null)]">{`x'${blob2hex(value, 8)}'`}</span>
    } else if (value === null) {  // NULL
        return <span class="text-[var(--data-null)]">NULL</span>
    } else if (typeof value === "string") {  // TEXT
        return <span class="text-[var(--data-string)]">{value.replaceAll("\\", "\\\\").replaceAll("\t", "\\t").replaceAll("\r", "\\r").replaceAll("\n", "\\n") || /* nbsp */"\u00a0"}</span>
    } else if (typeof value === "number") {  // REAL
        return <span class="text-[var(--data-number)]">{/^[+\-]?\d+$/.test("" + value) ? `${value}.0` : `${value}`}</span>
    } else {  // INTEGER
        return <span class="text-[var(--data-number)]">{"" + value}</span>
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
