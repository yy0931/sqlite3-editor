import { ColumnDef, ColumnDefEditor, Commit, Select } from "./components"
import { escapeSQLIdentifier } from "../main"
import { DispatchBuilder, EditorComponent, TitleComponent } from "."
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
}>
export declare const state: State

export let open: (tableName?: string) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName) => {
    if (tableName === undefined) { return }
    setState({ statement, tableName, statement2: "RENAME TO" })
}

export const Title: TitleComponent<State> = (props) =>
    <> {escapeSQLIdentifier(props.state.tableName)} <Select value={props.state.statement2} onChange={(value) => props.setState({ ...props.state, statement2: value })} options={{
        "RENAME TO": {},
        "RENAME COLUMN": {},
        "DROP COLUMN": {},
        "ADD COLUMN": {},
    }} />{" "}
        {props.state.statement2 === "RENAME TO" && <input placeholder="table-name" />}
        {(props.state.statement2 === "RENAME COLUMN" || props.state.statement2 === "DROP COLUMN") && <input placeholder="column-name" />}
        {props.state.statement2 === "RENAME COLUMN" && <>{" TO "}<input placeholder="column-name" /></>}
    </>

export const Editor: EditorComponent<State> = (props) => {
    const [columnDef, setColumnDef] = useState<ColumnDef>({ name: "", affinity: "TEXT", autoIncrement: false, notNull: false, primary: false, unique: false })
    return <pre style={{ paddingTop: "4px" }}>
        {props.state.statement2 === "ADD COLUMN" && <ColumnDefEditor value={columnDef} onChange={setColumnDef} />}
        <Commit onClick={() => {/* props.commit(`DROP TABLE ${escapeSQLIdentifier(props.state.tableName)}`, [], { refreshTableList: true }) */ }} />
    </pre>
}