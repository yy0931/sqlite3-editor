import { Commit, DataTypeInput, EditorDataType, parseTextareaValue } from "./components"
import { TableInfo, getTableInfo, escapeSQLIdentifier, type2color, sql } from "../main"
import produce from "immer"

export type State = Readonly<{
    statement: "INSERT"
    tableName: string
    tableInfo: TableInfo
    values: string[]
    dataTypes: EditorDataType[]
}>

export let open: () => Promise<void>

export const init = (setState: (newState: State) => void) => {
    open = async () => {
        const tableName = document.querySelector<HTMLSelectElement>("#tableSelect")!.value
        const tableInfo = await getTableInfo(tableName)
        setState({
            statement: "INSERT", tableName, tableInfo, values: tableInfo.map(() => ""), dataTypes: tableInfo.map(({ type }) => {
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
}

export const Title = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    const query = `INTO ${escapeSQLIdentifier(state.tableName)} (${state.tableInfo.map(({ name }) => name).map(escapeSQLIdentifier).join(", ")}) VALUES (${state.tableInfo.map(() => "?").join(", ")})`
    return <> {query}</>
}

export const Editor = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    const query = `INTO ${escapeSQLIdentifier(state.tableName)} (${state.tableInfo.map(({ name }) => name).map(escapeSQLIdentifier).join(", ")}) VALUES (${state.tableInfo.map(() => "?").join(", ")})`
    return <pre style={{ paddingTop: "4px" }}>
        {state.tableInfo.map(({ name }, i) => {
            return <>
                <div style={{ marginTop: "10px", marginBottom: "2px" }}>{name}</div><textarea autocomplete="off" style={{ width: "100%", height: "25px", resize: "vertical", display: "block", color: type2color(state.dataTypes[i]!) }} value={state.values[i]!} onChange={(ev) => { setState(produce(state, (d) => { d.values[i] = ev.currentTarget.value })) }} tabIndex={0}></textarea>
                AS
                <DataTypeInput value={state.dataTypes[i]!} onChange={(value) => { setState(produce(state, (d) => { d.dataTypes[i] = value })) }} />
            </>
        })}
        <Commit onClick={() => sql(`INSERT ${query}`, state.values.map((value, i) => parseTextareaValue(value, state.dataTypes[i]!)), "w+").then(() => refreshTable())} />
    </pre>
}
