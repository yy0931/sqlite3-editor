import { ColumnDef, ColumnDefEditor, Commit, printColumnDef, Select } from "./components"
import { escapeSQLIdentifier } from "../main"
import { DispatchBuilder, EditorComponent } from "."
import { useState } from "preact/hooks"

export const statement = "ALTER TABLE"
export type State = Readonly<{
    statement: typeof statement,
    tableName: string
}>
export declare const state: State

export let open: (tableName?: string) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName) => {
    if (tableName === undefined) { return }
    setState({ statement, tableName })
}

export const Editor: EditorComponent<State> = (props) => {
    const [columnDef, setColumnDef] = useState<ColumnDef>({ name: "", affinity: "TEXT", autoIncrement: false, notNull: false, primary: false, unique: false })
    const [statement2, setStatement2] = useState<
        | "RENAME TO"
        | "RENAME COLUMN"
        | "ADD COLUMN"
        | "DROP COLUMN"
    >("RENAME TO")
    const [newTableName, setNewTableName] = useState("")
    const [oldColumnName, setOldColumnName] = useState("")
    const [newColumnName, setNewColumnName] = useState("")
    return <pre>
        <h2>
            {props.statementSelect}{" "}{escapeSQLIdentifier(props.state.tableName)} <Select value={statement2} onChange={(value) => setStatement2(value)} options={{
                "RENAME TO": {},
                "RENAME COLUMN": {},
                "DROP COLUMN": {},
                "ADD COLUMN": {},
            }} />{" "}
            {statement2 === "RENAME TO" && <input placeholder="table-name" value={newTableName} onChange={(ev) => setNewTableName(ev.currentTarget.value)} />}
            {(statement2 === "RENAME COLUMN" || statement2 === "DROP COLUMN") && <input placeholder="column-name" value={oldColumnName} onChange={(ev) => setOldColumnName(ev.currentTarget.value)} />}
            {statement2 === "RENAME COLUMN" && <>{" TO "}<input placeholder="column-name" value={newColumnName} onChange={(ev) => setNewColumnName(ev.currentTarget.value)} /></>}
        </h2>
        {statement2 === "ADD COLUMN" && <ColumnDefEditor value={columnDef} onChange={setColumnDef} />}
        <Commit onClick={() => {
            let query = `ALTER TABLE ${escapeSQLIdentifier(props.state.tableName)} ${statement2} `
            switch (statement2) {
                case "RENAME TO": query += escapeSQLIdentifier(newTableName); break
                case "RENAME COLUMN": query += `${escapeSQLIdentifier(oldColumnName)} TO ${escapeSQLIdentifier(newColumnName)}`; break
                case "DROP COLUMN": query += escapeSQLIdentifier(oldColumnName); break
                case "ADD COLUMN": query += `${printColumnDef(columnDef)}`; break
            }
            props.commit(query, [], { refreshTableList: true, selectTable: statement2 === "RENAME TO" ? newTableName : undefined })
        }} />
    </pre>
}