import { useRef, Ref, useMemo, useLayoutEffect, useCallback, useState } from "preact/hooks"
import { type2color, blob2hex } from "./main"
import { SQLite3Value, TableInfo } from "./sql"
import { useEditorStore } from "./editor"
import zustand from "zustand"

export const useTableStore = zustand<{
    tableInfo: TableInfo
    records: readonly { readonly [key in string]: Readonly<SQLite3Value> }[]
    rowStart: bigint
    autoIncrement: boolean
    input: { readonly draftValue: string, readonly textarea: HTMLTextAreaElement | null } | null
    update: (tableInfo: TableInfo | null, records: readonly { readonly [key in string]: Readonly<SQLite3Value> }[], rowStart: bigint, autoIncrement: boolean) => void
}>()((set) => ({
    records: [],
    rowStart: 0n,
    tableInfo: [],
    autoIncrement: false,
    input: null,
    update: (tableInfo: TableInfo | null, records: readonly { readonly [key in string]: Readonly<SQLite3Value> }[], rowStart: bigint, autoIncrement: boolean) => {
        set({
            tableInfo: tableInfo ?? Object.keys(records[0] ?? {}).map((name) => ({ name, notnull: 0n, pk: 0n, type: "", cid: 0n, dflt_value: 0n })),
            records,
            rowStart,
            autoIncrement,
        })
    }
}))

export const Table = ({ tableName }: { tableName: string | undefined }) => {
    const alterTable = useEditorStore((state) => state.alterTable)

    const state = useTableStore()

    const columnWidths = useRef<(number | null)[]>(Object.keys(state.records[0] ?? {}).map(() => null))
    const tableRef = useRef() as Ref<HTMLTableElement>

    const selectedRow = useEditorStore((state) => state.statement === "DELETE" ? state.row : null)
    const selectedDataRow = useEditorStore((state) => state.statement === "UPDATE" ? state.row : null)
    const selectedDataColumn = useEditorStore((state) => state.statement === "UPDATE" ? state.column : null)

    // thead
    return <table ref={tableRef} className="viewer" style={{ background: "white", width: "max-content" }}>
        <thead>
            <tr>
                <th></th>
                {state.tableInfo.map(({ name, notnull, pk, type }, i) => <th
                    style={{ width: columnWidths.current[i] }}
                    className={tableName !== undefined ? "clickable" : ""}
                    onMouseMove={(ev) => {
                        const rect = ev.currentTarget.getBoundingClientRect()
                        if (rect.right - ev.clientX < 10) {
                            ev.currentTarget.classList.add("ew-resize")
                        } else {
                            ev.currentTarget.classList.remove("ew-resize")
                        }
                    }}
                    onMouseDown={(ev) => {
                        useEditorStore.getState().commitUpdate().then(() => {
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
                            } else if (tableName !== undefined) { // center
                                alterTable(tableName, name)
                            }
                        })
                    }}
                    onMouseLeave={(ev) => {
                        ev.currentTarget.classList.remove("ew-resize")
                    }}>
                    <code>
                        {name}
                        <span className="type">{(type ? (" " + type) : "") + (pk ? (state.autoIncrement ? " PRIMARY KEY AUTOINCREMENT" : " PRIMARY KEY") : "") + (notnull ? " NOT NULL" : "")}</span>
                    </code>
                </th>)}
            </tr>
        </thead>
        <tbody>
            {state.records.length === 0 && <tr>
                <td className="no-hover" style={{ display: "inline-block", height: "1.2em", cursor: "default" }}></td>
            </tr>}
            {state.records.map((record, i) => <TableRow selected={selectedRow === i} key={i} row={i} selectedColumn={selectedDataColumn} input={selectedDataRow === i ? state.input : null} tableName={tableName} tableInfo={state.tableInfo} record={record} columnWidth={columnWidths.current[i]!} rowNumber={state.rowStart + BigInt(i) + 1n} />)}
        </tbody>
    </table>
}

const TableRow = (props: { selected: boolean, readonly selectedColumn: string | null, input: { readonly draftValue: string, readonly textarea: HTMLTextAreaElement | null } | null, tableName: string | undefined, tableInfo: TableInfo, record: { readonly [key in string]: Readonly<SQLite3Value> }, rowNumber: bigint, row: number, columnWidth: number }) => {
    if (props.rowNumber <= 0) {
        throw new Error(props.rowNumber + "")
    }
    const delete_ = useEditorStore((state) => state.delete_)
    const update = useEditorStore((state) => state.update)

    const [cursorVisibility, setCursorVisibility] = useState(true)
    const onFocusOrMount = useCallback(() => { setCursorVisibility(true) }, [])
    const onBlurOrUnmount = useCallback(() => { setCursorVisibility(false) }, [])

    return useMemo(() => <tr className={props.selected ? "editing" : ""}>
        <td
            className={(props.tableName !== undefined ? "clickable" : "")}
            onMouseDown={(ev) => {
                useEditorStore.getState().commitUpdate().then(() => {
                    if (props.tableName !== undefined) { delete_(props.tableName, props.record, props.row).catch(console.error) }
                })
            }}>{props.rowNumber}</td>
        {props.tableInfo.map(({ name }) => {
            const value = props.record[name] as SQLite3Value
            const input = props.selectedColumn === name ? props.input : undefined
            return <td
                style={{ maxWidth: props.columnWidth }}
                className={(props.tableName !== undefined ? "clickable" : "") + " " + (input ? "editing" : "")}
                onMouseDown={(ev) => {
                    useEditorStore.getState().commitUpdate().then(() => {
                        if (props.tableName !== undefined) { update(props.tableName, name, props.record, props.row).catch(console.error) }
                    })
                }}>
                <pre className={input?.textarea && cursorVisibility ? "cursor-line" : ""} style={{ color: type2color(typeof value) }}>
                    <span className="value">{input?.draftValue ?? renderValue(value)}</span>
                    {input?.textarea && <MountInput element={input.textarea} onFocusOrMount={onFocusOrMount} onBlurOrUnmount={onBlurOrUnmount} />}
                </pre>
            </td>
        })}
    </tr>, [props.selected, props.selectedColumn, props.input?.draftValue, props.input?.textarea, props.tableName, props.tableInfo, props.record, props.rowNumber, cursorVisibility])  // excluded: props.columnWidth
}

const MountInput = (props: { element: HTMLTextAreaElement, onFocusOrMount: () => void, onBlurOrUnmount: () => void }) => {
    const ref = useRef() as Ref<HTMLSpanElement>
    useLayoutEffect(() => {
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
export const unsafeEscapeValue = (value: SQLite3Value) => {
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

export const renderValue = (value: SQLite3Value) => {
    if (value instanceof Uint8Array) {
        return `x'${blob2hex(value, 8)}'`
    } else if (value === null) {
        return "NULL"
    } else if (typeof value === "string") {
        return value.replaceAll("'", "''").replaceAll("\r", "\\r").replaceAll("\n", "\\n")
    } else if (typeof value === "number") {
        return /^[+\-]?\d+$/.test("" + value) ? "" + value + ".0" : "" + value
    } else {
        return "" + value
    }
}
