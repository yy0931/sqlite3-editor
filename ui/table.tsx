import { useRef, Ref, useMemo, useLayoutEffect, useCallback, useState, useEffect } from "preact/hooks"
import { useMainStore } from "./main"
import * as remote from "./remote"
import { useEditorStore } from "./editor"
import zustand from "zustand"
import type { ReadonlyDeep } from "type-fest"
import produce from "immer"
import { scrollbarWidth, ScrollbarY } from "./scrollbar"

export const useTableStore = zustand<{
    tableInfo: remote.TableInfo
    records: readonly { readonly [key in string]: Readonly<remote.SQLite3Value> }[]
    autoIncrement: boolean
    input: { readonly draftValue: string, readonly textarea: HTMLTextAreaElement | null } | null
}>()((set) => ({
    records: [],
    tableInfo: [],
    autoIncrement: false,
    input: null,
}))

/** useRef() but persists the value in the server. */
const persistentRef = <T extends unknown>(key: string, defaultValue: T) => {
    return useState(() => {
        let value: ReadonlyDeep<T> = remote.getState(key) ?? defaultValue as ReadonlyDeep<T>
        return {
            get current(): ReadonlyDeep<T> { return value },
            set current(newValue: ReadonlyDeep<T>) { remote.setState(key, value = newValue) },
        }
    })[0]
}

const defaultColumnWidth = 150

export const Table = ({ tableName }: { tableName: string | undefined }) => {
    const alterTable = useEditorStore((state) => state.alterTable)
    const visibleAreaTop = useMainStore((state) => Number(state.paging.visibleAreaTop))
    const pageSize = useMainStore((state) => Number(state.paging.pageSize))
    const numRecords = useMainStore((state) => Number(state.paging.numRecords))

    const state = useTableStore()

    const columnWidths = persistentRef<Record<string, (number | undefined | null)[]>>("columnWidths_v3", {})
    const tableRef = useRef() as Ref<HTMLTableElement>

    const selectedRow = useEditorStore((state) => state.statement === "DELETE" ? state.row : null)
    const selectedDataRow = useEditorStore((state) => state.statement === "UPDATE" ? state.row : null)
    const selectedDataColumn = useEditorStore((state) => state.statement === "UPDATE" ? state.column : null)

    const scrollbarRef = useRef() as Ref<ScrollbarY>
    useEffect(() => {
        tableRef.current?.addEventListener("wheel", (ev) => {
            ev.preventDefault()
            scrollbarRef.current!.wheel(ev.deltaY / 30)
        })
    }, [])

    return <>
        <div style={{ maxWidth: "100%", overflowX: "auto", width: "max-content" }}>
            <table ref={tableRef} className="viewer" style={{ paddingRight: scrollbarWidth, width: "max-content" }}>
                <thead>
                    <tr>
                        <th></th>
                        {state.tableInfo.map(({ name, notnull, pk, type }, i) => <th
                            style={{ width: columnWidths.current[tableName ?? ""]?.[i] ?? defaultColumnWidth }}
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
                                            columnWidths.current = produce(columnWidths.current, (d) => {
                                                if (!Array.isArray(d[tableName ?? ""])) {
                                                    d[tableName ?? ""] = []
                                                }
                                                d[tableName ?? ""]![i] = Math.max(50, ev.clientX - rect.left)
                                            })
                                            th.style.width = columnWidths.current[tableName ?? ""]![i]! + "px"
                                            for (const td of tableRef.current?.querySelectorAll<HTMLElement>(`td:nth-child(${i + 2})`) ?? []) {
                                                td.style.maxWidth = columnWidths.current[tableName ?? ""]![i]! + "px"
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
                    {state.records.map((record, row) => <TableRow selected={selectedRow === row} key={row} row={row} selectedColumn={selectedDataColumn} input={selectedDataRow === row ? state.input : null} tableName={tableName} tableInfo={state.tableInfo} record={record} columnWidths={columnWidths.current[tableName ?? ""] ?? []} rowNumber={BigInt(visibleAreaTop + row) + 1n} />)}
                </tbody>
            </table>
        </div>
        {// @ts-ignore
            <scrollbar-y
                ref={scrollbarRef}
                min={0}
                max={numRecords}
                size={pageSize}
                value={visibleAreaTop}
                style={{ width: scrollbarWidth, height: "100%", right: 0, top: 0, position: "absolute" }}
                onChange={() => { useMainStore.getState().setPaging({ visibleAreaTop: BigInt(Math.round(scrollbarRef.current!.value)) }) }} />}
    </>
}

const TableRow = (props: { selected: boolean, readonly selectedColumn: string | null, input: { readonly draftValue: string, readonly textarea: HTMLTextAreaElement | null } | null, tableName: string | undefined, tableInfo: remote.TableInfo, record: { readonly [key in string]: Readonly<remote.SQLite3Value> }, rowNumber: bigint, row: number, columnWidths: readonly (number | null | undefined)[] }) => {
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
        {props.tableInfo.map(({ name }, i) => {
            const value = props.record[name] as remote.SQLite3Value
            const input = props.selectedColumn === name ? props.input : undefined
            return <td
                style={{ maxWidth: props.columnWidths[i] ?? defaultColumnWidth }}
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

export const renderValue = (value: remote.SQLite3Value) => {
    if (value instanceof Uint8Array) {
        return `x'${blob2hex(value, 8)}'`
    } else if (value === null) {
        return "NULL"
    } else if (typeof value === "string") {
        return value.replaceAll("'", "''").replaceAll("\r", "\\r").replaceAll("\n", "\\n") || "\u00a0"
    } else if (typeof value === "number") {
        return /^[+\-]?\d+$/.test("" + value) ? "" + value + ".0" : "" + value
    } else {
        return "" + value
    }
}

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
