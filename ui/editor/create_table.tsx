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

    return <ul>{renderedColumnDefs.map((columnDef, i) =>
        <li key={i}>
            <ColumnDefEditor columnNameOnly={i === renderedColumnDefs.length - 1 && columnDef.name === ""} value={columnDef} onChange={(value) => {
                props.onChange(produce(renderedColumnDefs, (d) => {
                    d[i] = value
                    while (d.at(-1)?.name === "") { d.pop() }
                }))
            }} />
        </li>
    )}</ul>
}

export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async () => { setState({ statement: statement, strict: true, tableConstraints: "", tableName: "", withoutRowId: false }) }

export const Editor: EditorComponent<State> = (props) => {
    const [columnDefs, setColumnDefs] = useState<ColumnDef[]>([])
    return <>
        <h2>
            {props.statementSelect}{" "}<input placeholder="table-name" value={props.state.tableName} onInput={(ev) => { props.setState({ ...props.state, tableName: ev.currentTarget.value }) }}></input>(...)
            <Checkbox checked={props.state.withoutRowId} onChange={(checked) => { props.setState({ ...props.state, withoutRowId: checked }) }} style={{ marginLeft: "8px" }} text="WITHOUT ROWID" />
            <Checkbox checked={props.state.strict} onChange={(checked) => { props.setState({ ...props.state, strict: checked }) }} text="STRICT" />
        </h2>
        <div>
            <MultiColumnDefEditor value={columnDefs} onChange={setColumnDefs} />
            <textarea autocomplete="off" style={{ marginTop: "10px", height: "20vh" }} placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={props.state.tableConstraints} onInput={(ev) => { props.setState({ ...props.state, tableConstraints: ev.currentTarget.value }) }}></textarea>
            <Commit disabled={props.state.tableName === "" || columnDefs.length === 0} style={{ marginTop: "10px", marginBottom: "10px" }} onClick={() => {
                props.commit(`CREATE TABLE ${escapeSQLIdentifier(props.state.tableName)} (${columnDefs.map(printColumnDef).join(", ")}${props.state.tableConstraints.trim() !== "" ? (props.state.tableConstraints.trim().startsWith(",") ? props.state.tableConstraints : ", " + props.state.tableConstraints) : ""})${props.state.strict ? " STRICT" : ""}${props.state.withoutRowId ? " WITHOUT ROWID" : ""}`, [], { refreshTableList: true, selectTable: props.state.tableName }).then(() => {
                    open()
                    setColumnDefs([])
                })
            }} />
        </div>
    </>
}