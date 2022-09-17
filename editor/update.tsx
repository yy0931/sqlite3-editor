import { DataTypeInput, EditorDataType, parseTextareaValue } from "./components"
import { DataTypes, listUniqueConstraints, getTableList, blob2hex, escapeSQLIdentifier, unsafeEscapeValue, type2color, getTableName } from "../main"
import { useRef, Ref, useEffect } from "preact/hooks"
import * as insert from "./insert"
import { DispatchBuilder, EditorComponent, TitleComponent } from "."

export const statement = "UPDATE"
export type State = Readonly<{
    statement: typeof statement
    tableName: string
    column: string
    record: Record<string, DataTypes>
    textareaValue: string
    type: EditorDataType
    constraintChoices: readonly (readonly string[])[]
    selectedConstraint: number
    td: HTMLElement
}>
export declare const state: State

export let open: (column?: string, record?: Record<string, DataTypes>, td?: HTMLElement) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState) => open = async (column, record, td) => {
    if (column === undefined || record === undefined || td === undefined) { return }
    const tableName = getTableName()
    const value = record[column]
    const uniqueConstraints = await listUniqueConstraints(tableName)
    const withoutRowId = !!(await getTableList()).find(({ name }) => name === tableName)!.wr
    setState({
        statement,
        tableName,
        column,
        record,
        textareaValue: value instanceof Uint8Array ? blob2hex(value) : (value + ""),
        type: value === null ? "null" : value instanceof Uint8Array ? "blob" : typeof value === "number" ? "number" : "string",
        constraintChoices: uniqueConstraints.sort((a, b) => +b.primary - +a.primary)
            .map(({ columns }) => columns)
            .concat(withoutRowId ? [] : [["rowid"]])
            .filter((columns) => columns.every((column) => record[column] !== null)),
        selectedConstraint: 0,
        td,
    })
}

export const Title: TitleComponent<State> = (props) =>
    <> {escapeSQLIdentifier(props.state.tableName)} SET {escapeSQLIdentifier(props.state.column)} = ? <select value={props.state.selectedConstraint} onChange={(ev) => { props.setState({ ...props.state, selectedConstraint: +ev.currentTarget.value }) }}>{
        props.state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `WHERE ${column} = ${unsafeEscapeValue(props.state.record[column])}`).join(" ")}</option>)
    }</select></>

export const Editor: EditorComponent<State> = (props) => {
    const autoFocusRef = useRef(null) as Ref<HTMLTextAreaElement>
    useEffect(() => {
        autoFocusRef.current?.focus()
    }, [props.state.td])

    return <pre>
        <textarea ref={autoFocusRef} autocomplete="off" style={{ width: "100%", height: "20vh", resize: "none", color: type2color(props.state.type) }} value={props.state.textareaValue} onBlur={(ev) => {
            if (props.state.textareaValue === ev.currentTarget.value) { return }
            const columns = props.state.constraintChoices[props.state.selectedConstraint]!
            props.commit(`UPDATE ${escapeSQLIdentifier(props.state.tableName)} SET ${escapeSQLIdentifier(props.state.column)} = ? ` + columns.map((column) => `WHERE ${column} = ?`).join(" "), [parseTextareaValue(ev.currentTarget.value, props.state.type), ...columns.map((column) => props.state.record[column] as DataTypes)])
            insert.open()
        }}></textarea>
        AS
        <DataTypeInput value={props.state.type} onChange={(value) => { props.setState({ ...props.state, type: value }) }} />
    </pre>
}
