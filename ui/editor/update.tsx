import { Commit, DataEditor, DataTypeInput, EditorDataType, parseTextareaValue } from "./components"
import { blob2hex, escapeSQLIdentifier, unsafeEscapeValue, type2color } from "../main"
import { useRef, Ref, useEffect, useState } from "preact/hooks"
import * as insert from "./insert"
import { DispatchBuilder, EditorComponent } from "."
import SQLite3Client, { DataTypes } from "../sql"

export const statement = "UPDATE"
export type State = Readonly<{
    statement: typeof statement
    tableName: string
    column: string
    record: Record<string, DataTypes>
    textareaValue: string
    blobValue: Uint8Array | null
    type: EditorDataType
    constraintChoices: readonly (readonly string[])[]
    td: HTMLElement
    sql: SQLite3Client
}>
export declare const state: State

export let open: (tableName?: string, column?: string, record?: Record<string, DataTypes>, td?: HTMLElement) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName, column, record, td) => {
    if (tableName === undefined || column === undefined || record === undefined || td === undefined) { return }
    const value = record[column]
    const uniqueConstraints = await sql.listUniqueConstraints(tableName)
    const constraintChoices = ("rowid" in record ? [["rowid"]] : [])
        .concat(uniqueConstraints.sort((a, b) => +b.primary - +a.primary)
            .map(({ columns }) => columns)
            .filter((columns) => columns.every((column) => record[column] !== null)))
    if (constraintChoices.length === 0) { return }
    setState({
        statement,
        tableName,
        column,
        record,
        textareaValue: value instanceof Uint8Array ? "" : (value + ""),
        blobValue: value instanceof Uint8Array ? value : null,
        type: value === null ? "null" : value instanceof Uint8Array ? "blob" : (typeof value === "number" || typeof value === "bigint") ? "number" : "string",
        constraintChoices,
        td,
        sql,
    })
}

export const Editor: EditorComponent<State> = (props) => {
    const [selectedConstraint, setSelectedConstraint] = useState(0)
    const autoFocusRef = useRef(null) as Ref<HTMLTextAreaElement & HTMLInputElement>
    useEffect(() => {
        autoFocusRef.current?.focus?.()
    }, [props.state.td])

    return <>
        <h2>
            {props.statementSelect}{" "}{escapeSQLIdentifier(props.state.tableName)} SET {escapeSQLIdentifier(props.state.column)} = ? WHERE <select value={selectedConstraint} onChange={(ev) => { setSelectedConstraint(+ev.currentTarget.value) }}>{
                props.state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(props.state.record[column])}`).join(" AND ")}</option>)
            }</select>
        </h2>
        <div>
            <DataEditor
                ref={autoFocusRef}
                rows={5}
                type={props.state.type}
                textareaValue={props.state.textareaValue}
                setTextareaValue={(value) => props.setState({ ...props.state, textareaValue: value })}
                blobValue={props.state.blobValue}
                setBlobValue={(value) => props.setState({ ...props.state, blobValue: value })}
                sql={props.state.sql}
            />
            {"AS "}
            <DataTypeInput value={props.state.type} onChange={(value) => { props.setState({ ...props.state, type: value }) }} />
            <Commit style={{ marginTop: "10px", marginBottom: "10px" }} onClick={() => {
                // <textarea> replaces \r\n with \n
                const columns = props.state.constraintChoices[selectedConstraint]!
                props.commit(`UPDATE ${escapeSQLIdentifier(props.state.tableName)} SET ${escapeSQLIdentifier(props.state.column)} = ? WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`, [parseTextareaValue(props.state.textareaValue, props.state.blobValue, props.state.type), ...columns.map((column) => props.state.record[column] as DataTypes)], {}).then(() => {
                    insert.open(props.state.tableName)
                })
            }} />
        </div>
    </>
}