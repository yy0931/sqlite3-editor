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
    indexList: remote.IndexList
    /** index_info for each index in indexList */
    indexInfo: remote.IndexInfo[]
    autoIncrement: boolean
    records: readonly { readonly [key in string]: Readonly<remote.SQLite3Value> }[]
    input: { readonly draftValue: string, readonly textarea: HTMLTextAreaElement | null } | null
    listUniqueConstraints: () => {
        primary: boolean
        columns: string[]
    }[]
    getRecordSelectors: (record: Record<string, remote.SQLite3Value>) => string[][]
}>()((set, get) => ({
    tableInfo: [],
    indexList: [],
    indexInfo: [],
    autoIncrement: false,
    records: [],
    input: null,
    listUniqueConstraints: () => {
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
    },
    /** Enumerates the column tuples that uniquely select the record. */
    getRecordSelectors: (record: Record<string, remote.SQLite3Value>): string[][] => {
        const constraintChoices = ("rowid" in record ? [["rowid"]] : [])
            .concat(get().listUniqueConstraints().sort((a, b) => +b.primary - +a.primary)
                .map(({ columns }) => columns)
                .filter((columns) => columns.every((column) => record[column] !== null)))
        return [...new Set(constraintChoices.map((columns) => JSON.stringify(columns.sort((a, b) => a.localeCompare(b)))))].map((columns) => JSON.parse(columns))  // Remove duplicates
    },
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

export const Table = ({ tableName }: { tableName: string | undefined }) => {
    const alterTable = useEditorStore((state) => state.alterTable)
    const visibleAreaTop = useMainStore((state) => Number(state.paging.visibleAreaTop))
    const pageSize = useMainStore((state) => Number(state.paging.pageSize))
    const numRecords = useMainStore((state) => Number(state.paging.numRecords))

    const state = useTableStore()

    const columnWidthCaches = persistentRef<Record<string, (number | undefined | null)[]>>("columnWidths_v3", {})
    const getColumnWidths = () => {
        const result: number[] = []
        for (let i = 0; i < state.tableInfo.length; i++) {
            result.push(columnWidthCaches.current[tableName ?? ""]?.[i] ?? (state.tableInfo.length === 1 ? 250 : 150))
        }
        return result
    }
    const tableRef = useRef() as Ref<HTMLTableElement>

    const selectedRow = useEditorStore((state) => state.statement === "DELETE" ? state.row : null)
    const selectedDataRow = useEditorStore((state) => state.statement === "UPDATE" ? state.row : null)
    const selectedDataColumn = useEditorStore((state) => state.statement === "UPDATE" ? state.column : null)

    const scrollbarRef = useRef() as Ref<ScrollbarY>
    useEffect(() => {
        tableRef.current?.addEventListener("wheel", (ev) => {
            ev.preventDefault()
            scrollbarRef.current!.wheel(ev.deltaY / 30)
        }, { passive: false })
    }, [])

    return <>
        <div className="max-w-full overflow-x-auto w-max">
            <table ref={tableRef} className="viewer w-max border-collapse table-fixed bg-white" style={{ paddingRight: scrollbarWidth, boxShadow: "0 0 0px 2px #000000ad" }}>
                <thead className="text-black [background:var(--gutter-color)]" style={{ outline: "rgb(181, 181, 181) 1px solid" }}>
                    <tr>
                        <th className="font-normal select-none [padding-top:3px] [padding-bottom:3px] [padding-left:1em] [padding-right:1em]"></th>
                        {state.tableInfo.map(({ name, notnull, pk, type }, i) => <th
                            style={{ width: getColumnWidths()[i]! }}
                            className={"font-normal select-none " + (tableName !== undefined ? "clickable" : "")}
                            onMouseMove={(ev) => {
                                const rect = ev.currentTarget.getBoundingClientRect()
                                if (rect.right - ev.clientX < 20) {
                                    ev.currentTarget.classList.add("ew-resize")
                                } else {
                                    ev.currentTarget.classList.remove("ew-resize")
                                }
                            }}
                            onMouseDown={(ev) => {
                                useEditorStore.getState().commitUpdate().then(() => {
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
                                    } else if (tableName !== undefined) { // center
                                        alterTable(tableName, name)
                                    }
                                })
                            }}
                            onMouseLeave={(ev) => {
                                ev.currentTarget.classList.remove("ew-resize")
                            }}>
                            <code className="inline-block [word-break:break-word] [color:inherit] [font-family:inherit] [font-size:inherit]">
                                {name}
                                <span className="italic opacity-70">{(type ? (" " + type) : "") + (pk ? (state.autoIncrement ? " PRIMARY KEY AUTOINCREMENT" : " PRIMARY KEY") : "") + (notnull ? " NOT NULL" : "")}</span>
                            </code>
                        </th>)}
                    </tr>
                </thead>
                <tbody>
                    <tr>
                        <td className="[padding-left:10px] [padding-right:10px] [background-color:var(--gutter-color)]"></td>
                        <td className="relative text-right" colSpan={state.tableInfo.length} style={{ maxWidth: getColumnWidths().reduce((a, b) => a + b, 0) }}>
                            <FindWidget />
                        </td>
                    </tr>
                    {state.records.length === 0 && <tr>
                        <td className="overflow-hidden no-hover inline-block cursor-default [height:1.2em] [padding-left:10px] [padding-right:10px]" style={{ borderRight: "1px solid var(--td-border-color)" }}></td>
                    </tr>}
                    {state.records.map((record, row) => <TableRow selected={selectedRow === row} key={row} row={row} selectedColumn={selectedDataColumn} input={selectedDataRow === row ? state.input : null} tableName={tableName} tableInfo={state.tableInfo} record={record} columnWidths={getColumnWidths()} rowNumber={BigInt(visibleAreaTop + row) + 1n} />)}
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
                className="h-full right-0 top-0 absolute"
                style={{ width: scrollbarWidth }}
                onChange={() => { useMainStore.getState().setPaging({ visibleAreaTop: BigInt(Math.round(scrollbarRef.current!.value)) }) }} />}
    </>
}

const FindWidget = () => {
    const searchTerm = useMainStore((state) => state.searchTerm)
    const caseSensitive = useMainStore((state) => state.caseSensitive)
    const wholeWord = useMainStore((state) => state.wholeWord)
    const regex = useMainStore((state) => state.regex)
    const setViewerQuery = useMainStore((state) => state.setViewerQuery)
    const ref = useRef() as Ref<HTMLInputElement>

    useEffect(() => {
        window.addEventListener("keydown", (ev) => {
            if (ev.ctrlKey && ev.code === "KeyF") {
                ev.preventDefault()
                ref.current!.focus()
            }
        })
    }, [])

    return <div className="inline-block pl-1 pt-1 bg-gray-200 shadow-md whitespace-nowrap sticky right-3">
        <input ref={ref} className="mr-1" placeholder="Find" value={searchTerm} onChange={(ev) => setViewerQuery({ searchTerm: ev.currentTarget.value })} />
        <span className="[font-size:130%] align-middle text-gray-600 hover:bg-gray-300 select-none [padding:2px] [border-radius:1px] inline-block cursor-pointer" style={caseSensitive ? { background: "rgba(66, 159, 202, 0.384)", color: "black" } : {}} onClick={() => setViewerQuery({ caseSensitive: !caseSensitive })}>
            <svg className="[width:1em] [height:1em]"><use xlinkHref="#case-sensitive" /></svg>
        </span>
        <span className="[font-size:130%] align-middle text-gray-600 hover:bg-gray-300 select-none [padding:2px] [border-radius:1px] inline-block cursor-pointer" style={wholeWord ? { background: "rgba(66, 159, 202, 0.384)", color: "black" } : {}} onClick={() => setViewerQuery({ wholeWord: !wholeWord })}>
            <svg className="[width:1em] [height:1em]"><use xlinkHref="#whole-word" /></svg>
        </span>
        <span className="[font-size:130%] align-middle text-gray-600 hover:bg-gray-300 select-none [padding:2px] [border-radius:1px] inline-block cursor-pointer" style={regex ? { background: "rgba(66, 159, 202, 0.384)", color: "black" } : {}} onClick={() => setViewerQuery({ regex: !regex })}>
            <svg className="[width:1em] [height:1em]"><use xlinkHref="#regex" /></svg>
        </span>
    </div>
}

const TableRow = (props: { selected: boolean, readonly selectedColumn: string | null, input: { readonly draftValue: string, readonly textarea: HTMLTextAreaElement | null } | null, tableName: string | undefined, tableInfo: remote.TableInfo, record: { readonly [key in string]: Readonly<remote.SQLite3Value> }, rowNumber: bigint, row: number, columnWidths: readonly number[] }) => {
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
            className={"[padding-left:10px] [padding-right:10px] [background-color:var(--gutter-color)] overflow-hidden sticky left-0 whitespace-nowrap text-right text-black select-none " + (props.tableName !== undefined ? "clickable" : "")}
            style={{ borderRight: "1px solid var(--td-border-color)" }}
            onMouseDown={(ev) => {
                useEditorStore.getState().commitUpdate().then(() => {
                    if (props.tableName !== undefined) { delete_(props.tableName, props.record, props.row).catch(console.error) }
                })
            }}>{props.rowNumber}</td>
        {props.tableInfo.map(({ name }, i) => {
            const value = props.record[name] as remote.SQLite3Value
            const input = props.selectedColumn === name ? props.input : undefined
            return <td
                className={"[padding-left:10px] [padding-right:10px] overflow-hidden " + (props.tableName !== undefined ? "clickable" : "") + " " + (input ? "editing" : "")}
                style={{ borderRight: "1px solid var(--td-border-color)", maxWidth: props.columnWidths[i], borderBottom: "1px solid var(--td-border-color)" }}
                onMouseDown={(ev) => {
                    const editorState = useEditorStore.getState()
                    if (editorState.statement === "UPDATE" && editorState.row === props.row && editorState.column === name) { return }
                    editorState.commitUpdate().then(() => {
                        if (props.tableName !== undefined) { update(props.tableName, name, props.row) }
                    })
                }}>
                <pre className={"overflow-hidden text-ellipsis whitespace-nowrap [max-width:50em] [font-size:inherit] " + (input?.textarea && cursorVisibility ? "cursor-line" : "")} style={{ color: type2color(typeof value) }}>
                    <span className="select-none">{input?.draftValue ?? renderValue(value)}</span>
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
