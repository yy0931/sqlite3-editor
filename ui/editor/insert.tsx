import { Commit, DataTypeInput, EditorDataType, parseTextareaValue } from "./components"
import { escapeSQLIdentifier, type2color } from "../main"
import produce from "immer"
import type { DispatchBuilder, EditorComponent } from "."
import { TableInfo } from "../sql"

export const statement = "INSERT"
export type State = Readonly<{
    statement: typeof statement
    tableName: string
    tableInfo: TableInfo
    values: string[]
    dataTypes: EditorDataType[]
}>
export declare const state: State

export let open: (tableName?: string) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName) => {
    if (tableName === undefined) { return }
    const tableInfo = await sql.getTableInfo(tableName)
    setState({
        statement, tableName, tableInfo, values: tableInfo.map(() => ""), dataTypes: tableInfo.map(({ type }) => {
            type = type.toLowerCase()
            if (type === "numeric" || type === "real" || type === "int" || type === "integer") {
                return "number"
            } else if (type === "text") {
                return "string"
            } else if (type === "null" || type === "blob") {
                return type
            } else {
                return "string"
            }
        })
    })
}

export const Editor: EditorComponent<State> = (props) => {
    const query = `INTO ${escapeSQLIdentifier(props.state.tableName)} (${props.state.tableInfo.map(({ name }) => name).map(escapeSQLIdentifier).join(", ")}) VALUES (${props.state.tableInfo.map(() => "?").join(", ")})`
    return <>
        <h2>
            {props.statementSelect}{" "}{query}
        </h2>
        <div>
            <ul>
                {props.state.tableInfo.map(({ name }, i) => {
                    return <li>
                        <div style={{ marginRight: "1em" }}>{name}</div>
                        <textarea
                            autocomplete="off" rows={1} style={{ width: "100%", resize: "vertical", display: "block", color: type2color(props.state.dataTypes[i]!) }}
                            value={props.state.dataTypes[i]! === "null" ? "NULL" : props.state.values[i]!}
                            onInput={(ev) => { props.setState(produce(props.state, (d) => { d.values[i] = ev.currentTarget.value })) }}
                            disabled={props.state.dataTypes[i]! === "null"}
                            tabIndex={0}></textarea>
                        {"AS "}<DataTypeInput value={props.state.dataTypes[i]!} onChange={(value) => { props.setState(produce(props.state, (d) => { d.dataTypes[i] = value })) }} />
                    </li>
                })}
            </ul>
            <Commit style={{ marginTop: "10px", marginBottom: "10px" }} onClick={() => {
                props.commit(`INSERT ${query}`, props.state.values.map((value, i) => parseTextareaValue(value, props.state.dataTypes[i]!)), {}).then(() => {
                    open(props.state.tableName)
                })
            }} />
        </div>
    </>
}