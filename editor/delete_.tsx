import { escapeSQLIdentifier, unsafeEscapeValue } from "../main"
import { DispatchBuilder, EditorComponent } from "."
import { DataTypes } from "../sql"
import { Commit } from "./components"

export const statement = "DELETE"
export type State = Readonly<{
    statement: typeof statement
    tableName: string
    record: Record<string, DataTypes>
    constraintChoices: readonly (readonly string[])[]
    selectedConstraint: number
    tr: HTMLElement
}>
export declare const state: State

export let open: (tableName?: string, record?: Record<string, DataTypes>, tr?: HTMLElement) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName, record, tr) => {
    if (tableName === undefined || record === undefined || tr === undefined) { return }
    const uniqueConstraints = await sql.listUniqueConstraints(tableName)
    setState({
        statement,
        tableName,
        record,
        constraintChoices: ("rowid" in record ? [["rowid"]] : [])
            .concat(uniqueConstraints.sort((a, b) => +b.primary - +a.primary)
                .map(({ columns }) => columns)
                .filter((columns) => columns.every((column) => record[column] !== null))),
        selectedConstraint: 0,
        tr: tr,
    })
}

export const Editor: EditorComponent<State> = (props) => {
    const columns = props.state.constraintChoices[props.state.selectedConstraint]!
    return <pre>
        <h2>
            {props.statementSelect}{" "}FROM {escapeSQLIdentifier(props.state.tableName)} WHERE <select value={props.state.selectedConstraint} onChange={(ev) => { props.setState({ ...props.state, selectedConstraint: +ev.currentTarget.value }) }}>{
                props.state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(props.state.record[column])}`).join(" AND ")}</option>)
            }</select>
        </h2>
        <Commit onClick={() => props.commit(`DELETE FROM ${escapeSQLIdentifier(props.state.tableName)} WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`, [...columns.map((column) => props.state.record[column] as DataTypes)], {})} />
    </pre>
}
