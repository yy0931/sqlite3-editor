import { Commit, DataEditor, DataTypeInput, EditorDataType, parseTextareaValue } from "./components"
import { escapeSQLIdentifier, type2color } from "../main"
import produce from "immer"
import type { DispatchBuilder, EditorComponent } from "."
import SQLite3Client, { TableInfo } from "../sql"

export const statement = "INSERT"
export type State = Readonly<{
    statement: typeof statement
    tableName: string
    tableInfo: TableInfo
    textareaValues: string[]
    blobValues: (Uint8Array | null)[]
    dataTypes: EditorDataType[]
    sql: SQLite3Client
}>
export declare const state: { _: State }

export let open: (tableName?: string) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName) => {
    if (tableName === undefined) { return }
    const tableInfo = await sql.getTableInfo(tableName)
    setState({
        statement, tableName, tableInfo, textareaValues: tableInfo.map(() => ""), blobValues: tableInfo.map(() => null), dataTypes: tableInfo.map(({ type }) => {
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
        }), sql,
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
                        <DataEditor
                            type={props.state.dataTypes[i]!}
                            rows={1}
                            style={{ width: "100%", resize: "vertical", display: "block", color: type2color(props.state.dataTypes[i]!) }}
                            textareaValue={props.state.textareaValues[i]!}
                            onTextareaValueChange={(value) => { props.setState(produce(props.state, (d) => { d.textareaValues[i] = value })) }}
                            blobValue={props.state.blobValues[i]!}
                            onBlobValueChange={(value) => { props.setState(produce(props.state, (d) => { d.blobValues[i] = value })) }}
                            tabIndex={0}
                            sql={props.state.sql} />
                        {"AS "}<DataTypeInput value={props.state.dataTypes[i]!} onChange={(value) => { props.setState(produce(props.state, (d) => { d.dataTypes[i] = value })) }} />
                    </li>
                })}
            </ul>
            <Commit style={{ marginTop: "10px", marginBottom: "10px" }} onClick={() => {
                props.commit(`INSERT ${query}`, props.state.textareaValues.map((value, i) => parseTextareaValue(value, props.state.blobValues[i]!, props.state.dataTypes[i]!)), { scrollToBottom: true }).then(() => {
                    open(props.state.tableName)
                })
            }} />
        </div>
    </>
}