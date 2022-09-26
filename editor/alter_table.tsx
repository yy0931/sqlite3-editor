import { ColumnDef, ColumnDefEditor, Commit, printColumnDef, Select } from "./components"
import { escapeSQLIdentifier } from "../main"
import { DispatchBuilder, EditorComponent } from "."
import { useState } from "preact/hooks"

export const statement = "ALTER TABLE"
export type State = Readonly<{
    statement: typeof statement,
    tableName: string
    statement2:
    | "RENAME TO"
    | "RENAME COLUMN"
    | "ADD COLUMN"
    | "DROP COLUMN"
    oldColumnName: string
}>
export declare const state: State

export let open: (tableName?: string, column?: string) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName, column) => {
    if (tableName === undefined) { return }
    setState({ statement, tableName, statement2: column ? "RENAME COLUMN" : "RENAME TO", oldColumnName: column ?? "" })
}

export const Editor: EditorComponent<State> = (props) => {
    const [columnDef, setColumnDef] = useState<ColumnDef>({ name: "", affinity: "TEXT", autoIncrement: false, notNull: false, primary: false, unique: false })
    const [newTableName, setNewTableName] = useState("")
    const [newColumnName, setNewColumnName] = useState("")
    return <>
        <h2>
            {props.statementSelect}{" "}{escapeSQLIdentifier(props.state.tableName)} <Select value={props.state.statement2} onChange={(value) => props.setState({ ...props.state, statement2: value })} options={{
                "RENAME TO": {},
                "RENAME COLUMN": {},
                "DROP COLUMN": {},
                "ADD COLUMN": {},
            }} />{" "}
            {props.state.statement2 === "RENAME TO" && <input placeholder="table-name" value={newTableName} onChange={(ev) => setNewTableName(ev.currentTarget.value)} />}
            {(props.state.statement2 === "RENAME COLUMN" || props.state.statement2 === "DROP COLUMN") && <input placeholder="column-name" value={props.state.oldColumnName} onChange={(ev) => props.setState({ ...state, oldColumnName: ev.currentTarget.value })} />}
            {props.state.statement2 === "RENAME COLUMN" && <>{" TO "}<input placeholder="column-name" value={newColumnName} onChange={(ev) => setNewColumnName(ev.currentTarget.value)} /></>}
        </h2>
        <div>
            {props.state.statement2 === "ADD COLUMN" && <ColumnDefEditor value={columnDef} onChange={setColumnDef} />}
            <Commit onClick={() => {
                let query = `ALTER TABLE ${escapeSQLIdentifier(props.state.tableName)} ${props.state.statement2} `
                switch (props.state.statement2) {
                    case "RENAME TO": query += escapeSQLIdentifier(newTableName); break
                    case "RENAME COLUMN": query += `${escapeSQLIdentifier(props.state.oldColumnName)} TO ${escapeSQLIdentifier(newColumnName)}`; break
                    case "DROP COLUMN": query += escapeSQLIdentifier(props.state.oldColumnName); break
                    case "ADD COLUMN": query += `${printColumnDef(columnDef)}`; break
                }
                props.commit(query, [], { refreshTableList: true, selectTable: props.state.statement2 === "RENAME TO" ? newTableName : undefined })
            }} />
        </div>
    </>
}