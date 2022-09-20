import { useState } from "preact/hooks"
import produce from "immer"
import { DispatchBuilder, EditorComponent } from "."
import { escapeSQLIdentifier } from "../main"
import { Checkbox, ColumnDef, ColumnDefEditor, Commit, printColumnDef, Select } from "./components"

export const statement = "CREATE TABLE"

export type State = Readonly<{
    statement: typeof statement
    tableName: string
    withoutRowId: boolean
    strict: boolean
    tableConstraints: string
}>
export declare const state: State

export let open: () => Promise<void>

const MultiColumnDefEditor = (props: { value: ColumnDef[], onChange: (value: ColumnDef[]) => void }) => {
    const renderedColumnDefs = [...props.value]
    while (renderedColumnDefs.at(-1)?.name === "") { renderedColumnDefs.pop() }
    if (renderedColumnDefs.length === 0 || renderedColumnDefs.at(-1)!.name !== "") {
        renderedColumnDefs.push({ name: "", affinity: "TEXT", autoIncrement: false, notNull: false, primary: false, unique: false })
    }

    return <>{renderedColumnDefs.map((columnDef, i) =>
        <div style={{ marginBottom: "10px" }} key={i}>
            <ColumnDefEditor value={columnDef} onChange={(value) => {
                props.onChange(produce(renderedColumnDefs, (d) => {
                    d[i] = value
                    if (d.at(-1)?.name === "") { d.pop() }
                }))
            }} />
        </div>
    )}</>
}

export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async () => { setState({ statement: statement, strict: true, tableConstraints: "", tableName: "", withoutRowId: false }) }

export const Editor: EditorComponent<State> = (props) => {
    const [columnDefs, setColumnDefs] = useState<ColumnDef[]>([])
    return <pre>
        <h2>
            {props.statementSelect}{" "}<input placeholder="table-name" value={props.state.tableName} onChange={(ev) => { props.setState({ ...props.state, tableName: ev.currentTarget.value }) }}></input>(...)
            <Checkbox checked={props.state.withoutRowId} onChange={(checked) => { props.setState({ ...props.state, withoutRowId: checked }) }} style={{ marginLeft: "8px" }} text="WITHOUT ROWID" />
            <Checkbox checked={props.state.strict} onChange={(checked) => { props.setState({ ...props.state, strict: checked }) }} text="STRICT" />
        </h2>
        <MultiColumnDefEditor value={columnDefs} onChange={setColumnDefs} />
        <textarea autocomplete="off" style={{ marginTop: "15px", width: "100%", height: "20vh", resize: "none" }} placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={props.state.tableConstraints} onChange={(ev) => { props.setState({ ...props.state, tableConstraints: ev.currentTarget.value }) }}></textarea><br></br>
        <Commit onClick={() => props.commit(`CREATE TABLE ${escapeSQLIdentifier(props.state.tableName)} (${columnDefs.map(printColumnDef).join(", ")}${props.state.tableConstraints.trim() !== "" ? (props.state.tableConstraints.trim().startsWith(",") ? props.state.tableConstraints : ", " + props.state.tableConstraints) : ""})${props.state.strict ? " STRICT" : ""}${props.state.withoutRowId ? " WITHOUT ROWID" : ""}`, [], { refreshTableList: true, selectTable: props.state.tableName })} />
    </pre>
}
