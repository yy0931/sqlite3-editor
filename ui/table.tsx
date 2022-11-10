import { useRef, Ref } from "preact/hooks"
import { type2color, blob2hex, unsafeEscapeValue } from "./main"
import { DataTypes, TableInfo } from "./sql"
import { useStore } from "./editor"
import type SQLite3Client from "./sql"

export type TableProps = { tableName: string | null, tableInfo: TableInfo | null, records: Record<string, DataTypes>[], rowStart: bigint, autoIncrement: boolean, sql: SQLite3Client }

export const Table = ({ records, rowStart, tableInfo, tableName, autoIncrement, sql }: TableProps) => {
    const store = useStore()

    if (tableInfo === null) {
        tableInfo = Object.keys(records[0] ?? {}).map((name) => ({ name, notnull: 0n, pk: 0n, type: "", cid: 0n, dflt_value: 0n }))
    }

    const columnWidths = useRef<(number | null)[]>(Object.keys(records[0] ?? {}).map(() => null))
    const tableRef = useRef() as Ref<HTMLTableElement>

    // thead
    return <table ref={tableRef} className="viewer" style={{ background: "white", width: "max-content" }}>
        <thead>
            <tr>
                <th></th>
                {tableInfo.map(({ name, notnull, pk, type }, i) => <th
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
                            store.alterTable(tableName, name)
                        }
                    }}
                    onMouseLeave={(ev) => {
                        ev.currentTarget.classList.remove("ew-resize")
                    }}>
                    <code>
                        {name}
                        <span className="type">{(type ? (" " + type) : "") + (pk ? (autoIncrement ? " PRIMARY KEY AUTOINCREMENT" : " PRIMARY KEY") : "") + (notnull ? " NOT NULL" : "")}</span>
                    </code>
                </th>)}
            </tr>
        </thead>
        <tbody>
            {records.length === 0 && <tr>
                <td className="no-hover" style={{ display: "inline-block", height: "1.2em", cursor: "default" }}></td>
            </tr>}
            {records.map((record, i) => <tr>
                <td
                    className={tableName !== null ? "clickable" : ""}
                    onClick={(ev) => { if (tableName !== null) { store.delete_(tableName, record, ev.currentTarget.parentElement as HTMLTableRowElement, sql).catch(console.error) } }}>{rowStart + BigInt(i) + 1n}</td>
                {tableInfo!.map(({ name }, i) => {
                    const value = record[name] as DataTypes
                    return <td
                        style={{ maxWidth: columnWidths.current[i] }}
                        className={tableName !== null ? "clickable" : ""}
                        onClick={(ev) => { if (tableName !== null) { store.update(tableName, name, record, ev.currentTarget, sql).catch(console.error) } }}>
                        <pre style={{ color: type2color(typeof value) }}>
                            {value instanceof Uint8Array ? `x'${blob2hex(value, 8)}'` :
                                value === null ? "NULL" :
                                    typeof value === "string" ? unsafeEscapeValue(value) :
                                        typeof value === "number" ? (/^[+\-]?\d+$/.test("" + value) ? "" + value + ".0" : "" + value) :
                                            "" + value}
                        </pre>
                    </td>
                })}
            </tr>)}
        </tbody>
    </table>
}
