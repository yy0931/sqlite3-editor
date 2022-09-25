import { DataTypeInput, EditorDataType, parseTextareaValue } from "./components"
import { blob2hex, escapeSQLIdentifier, unsafeEscapeValue, type2color } from "../main"
import { useRef, Ref, useEffect, useState } from "preact/hooks"
import * as createTable from "./create_table"
import { DispatchBuilder, EditorComponent } from "."
import { DataTypes } from "../sql"

export const statement = "UPDATE"
export type State = Readonly<{
    statement: typeof statement
    tableName: string
    column: string
    record: Record<string, DataTypes>
    textareaValue: string
    type: EditorDataType
    constraintChoices: readonly (readonly string[])[]
    td: HTMLElement
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
        textareaValue: value instanceof Uint8Array ? blob2hex(value) : (value + ""),
        type: value === null ? "null" : value instanceof Uint8Array ? "blob" : typeof value === "number" ? "number" : "string",
        constraintChoices,
        td,
    })
}

export const Editor: EditorComponent<State> = (props) => {
    const [selectedConstraint, setSelectedConstraint] = useState(0)
    const autoFocusRef = useRef(null) as Ref<HTMLTextAreaElement>
    useEffect(() => {
        autoFocusRef.current?.focus()
    }, [props.state.td])

    return <pre>
        <h2>
            {props.statementSelect}{" "}{escapeSQLIdentifier(props.state.tableName)} SET {escapeSQLIdentifier(props.state.column)} = ? WHERE <select value={selectedConstraint} onChange={(ev) => { setSelectedConstraint(+ev.currentTarget.value) }}>{
                props.state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(props.state.record[column])}`).join(" AND ")}</option>)
            }</select>
        </h2>
        <textarea ref={autoFocusRef} autocomplete="off" style={{ width: "100%", height: "20vh", resize: "none", color: type2color(props.state.type) }} value={props.state.textareaValue} onBlur={(ev) => {
            // <textarea> replaces \r\n with \n
            if (props.state.textareaValue.replaceAll(/\r/g, "") === ev.currentTarget.value.replaceAll(/\r/g, "")) { return }
            const columns = props.state.constraintChoices[selectedConstraint]!
            props.commit(`UPDATE ${escapeSQLIdentifier(props.state.tableName)} SET ${escapeSQLIdentifier(props.state.column)} = ? WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`, [parseTextareaValue(ev.currentTarget.value, props.state.type), ...columns.map((column) => props.state.record[column] as DataTypes)], {})
            createTable.open()
        }}></textarea><br />
        AS
        <DataTypeInput value={props.state.type} onChange={(value) => { props.setState({ ...props.state, type: value }) }} />
    </pre>
}
