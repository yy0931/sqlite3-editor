import { DataTypeInput, EditorDataType, parseTextareaValue } from "."
import { DataTypes, listUniqueConstraints, getTableList, blob2hex, escapeSQLIdentifier, unsafeEscapeValue, sql, type2color } from "../main"
import { useRef, Ref, useEffect } from "preact/hooks"
import * as insert from "./insert"

export type State = Readonly<{
    statement: "UPDATE"
    tableName: string
    column: string
    record: Record<string, DataTypes>
    textareaValue: string
    type: EditorDataType
    constraintChoices: readonly (readonly string[])[]
    selectedConstraint: number
    td: HTMLElement
}>

export let open: (column: string, record: Record<string, DataTypes>, td: HTMLElement) => Promise<void>

export const init = (setState: (newState: State) => void) => {
    open = async (column, record, td) => {
        const tableName = document.querySelector<HTMLSelectElement>("#tableSelect")!.value
        const value = record[column]
        const uniqueConstraints = await listUniqueConstraints(tableName)
        const withoutRowId = !!(await getTableList()).find(({ name }) => name === tableName)!.wr
        setState({
            statement: "UPDATE",
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

    return <></>
}

export const Title = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    return <> {escapeSQLIdentifier(state.tableName)} SET {escapeSQLIdentifier(state.column)} = ? <select value={state.selectedConstraint} onChange={(ev) => { setState({ ...state, selectedConstraint: +ev.currentTarget.value }) }}>{
        state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `WHERE ${column} = ${unsafeEscapeValue(state.record[column])}`).join(" ")}</option>)
    }</select></>
}

export const Editor = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    const autoFocusRef = useRef(null) as Ref<HTMLTextAreaElement>
    useEffect(() => {
        autoFocusRef.current?.focus()
    }, [state.td])

    return <pre>
        <textarea ref={autoFocusRef} autocomplete="off" style={{ width: "100%", height: "20vh", resize: "none", color: type2color(state.type) }} value={state.textareaValue} onBlur={(ev) => {
            if (state.textareaValue === ev.currentTarget.value) { return }
            const columns = state.constraintChoices[state.selectedConstraint]!
            sql(`UPDATE ${escapeSQLIdentifier(state.tableName)} SET ${escapeSQLIdentifier(state.column)} = ? ` + columns.map((column) => `WHERE ${column} = ?`).join(" "), [parseTextareaValue(ev.currentTarget.value, state.type), ...columns.map((column) => state.record[column] as DataTypes)], "w+")
                .then(() => refreshTable())
                .catch(console.error)
            insert.open()
        }}></textarea>
        AS <DataTypeInput value={state.type} onChange={(value) => { setState({ ...state, type: value }) }} />
    </pre>
}
