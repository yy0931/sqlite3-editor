import { useRef, Ref, useMemo } from "preact/hooks"
import { type2color, blob2hex, unsafeEscapeValue } from "./main"
import { DataTypes, TableInfo } from "./sql"
import { useEditorStore } from "./editor"
import type SQLite3Client from "./sql"
import zustand from "zustand"

export const useTableStore = zustand<{
    tableInfo: TableInfo
    records: readonly { readonly [key in string]: Readonly<DataTypes> }[]
    rowStart: bigint
    autoIncrement: boolean
    update: (tableInfo: TableInfo | null, records: readonly { readonly [key in string]: Readonly<DataTypes> }[], rowStart: bigint, autoIncrement: boolean) => void
}>()((set) => ({
    records: [],
    rowStart: 0n,
    tableInfo: [],
    autoIncrement: false,
    update: (tableInfo: TableInfo | null, records: readonly { readonly [key in string]: Readonly<DataTypes> }[], rowStart: bigint, autoIncrement: boolean) => {
        set({
            tableInfo: tableInfo ?? Object.keys(records[0] ?? {}).map((name) => ({ name, notnull: 0n, pk: 0n, type: "", cid: 0n, dflt_value: 0n })),
            records,
            rowStart,
            autoIncrement,
        })
    }
}))

export const Table = ({ tableName, sql }: { tableName: string, sql: SQLite3Client }) => {
    const alterTable = useEditorStore((state) => state.alterTable)

    const state = useTableStore()

    const columnWidths = useRef<(number | null)[]>(Object.keys(state.records[0] ?? {}).map(() => null))
    const tableRef = useRef() as Ref<HTMLTableElement>

    // thead
    return <table ref={tableRef} className="viewer" style={{ background: "white", width: "max-content" }}>
        <thead>
            <tr>
                <th></th>
                {state.tableInfo.map(({ name, notnull, pk, type }, i) => <th
                    style={{ width: columnWidths.current[i] }}
                    className={tableName !== null ? "clickable" : ""}
                    onMouseMove={(ev) => {
                        const rect = ev.currentTarget.getBoundingClientRect()
                        if (rect.right - ev.clientX < 10) {
                            ev.currentTarget.classList.add("ew-resize")
                        } else {
                            ev.currentTarget.classList.remove("ew-resize")
                        }
                    }}
                    onMouseDown={(ev) => {
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
                        } else if (tableName !== null) { // center
                            alterTable(tableName, name)
                        }
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
            {state.records.map((record, i) => <TableRow key={i} tableName={tableName} tableInfo={state.tableInfo} record={record} columnWidth={columnWidths.current[i]!} rowNumber={state.rowStart + BigInt(i) + 1n} sql={sql} />)}
        </tbody>
    </table>
}

const TableRow = (props: { tableName: string, tableInfo: TableInfo, record: { readonly [key in string]: Readonly<DataTypes> }, sql: SQLite3Client, rowNumber: bigint, columnWidth: number }) => {
    const delete_ = useEditorStore((state) => state.delete_)
    const update = useEditorStore((state) => state.update)

    return useMemo(() => <tr>
        <td
            className={props.tableName !== null ? "clickable" : ""}
            onClick={(ev) => { if (props.tableName !== null) { delete_(props.tableName, props.record, ev.currentTarget.parentElement as HTMLTableRowElement, props.sql).catch(console.error) } }}>{props.rowNumber}</td>
        {props.tableInfo.map(({ name }, i) => {
            const value = props.record[name] as DataTypes
            return <td
                style={{ maxWidth: props.columnWidth }}
                className={props.tableName !== null ? "clickable" : ""}
                onClick={(ev) => { if (props.tableName !== null) { update(props.tableName, name, props.record, ev.currentTarget, props.sql).catch(console.error) } }}>
                <pre style={{ color: type2color(typeof value) }}>
                    <span className="value">{renderValue(value)}</span>
                </pre>
            </td>
        })}
    </tr>, [props.tableName, props.tableInfo, props.record, props.sql, props.rowNumber])  // excluded: props.columnWidth
}

export const renderValue = (value: DataTypes) => {
    if (value instanceof Uint8Array) {
        return `x'${blob2hex(value, 8)}'`
    } else if (value === null) {
        return "NULL"
    } else if (typeof value === "string") {
        return unsafeEscapeValue(value)
    } else if (typeof value === "number") {
        return /^[+\-]?\d+$/.test("" + value) ? "" + value + ".0" : "" + value
    } else {
        return "" + value
    }
}
