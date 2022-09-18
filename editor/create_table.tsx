import { useState, useRef } from "preact/hooks"
import { useImmer } from "use-immer"
import { DispatchBuilder, EditorComponent, TitleComponent } from "."
import { escapeSQLIdentifier } from "../main"
import { Checkbox, Commit, Select } from "./components"

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

const TableColumnSchemaEditor = (props: { schema: preact.RefObject<string> }) => {
    const [names, setNames] = useState<string[]>([])
    const [affinity, setAffinity] = useImmer(new Map<number, string>())
    const [primary, setPrimary] = useImmer(new Map<number, boolean>())
    const [autoIncrement, setAutoIncrement] = useImmer(new Map<number, boolean>())
    const [unique, setUnique] = useImmer(new Map<number, boolean>())
    const [notNull, setNotNull] = useImmer(new Map<number, boolean>())

    props.schema.current = names.map((name, i) => `${escapeSQLIdentifier(name)} ${affinity.get(i) ?? "TEXT"}${primary.get(i) ? " PRIMARY KEY" : ""}${autoIncrement.get(i) ? " AUTOINCREMENT" : ""}${unique.get(i) ? " UNIQUE" : ""}${notNull.get(i) ? " NOT NULL" : ""}`).join(", ")

    return <>{names.concat([""]).map((column, i) => {
        return <div style={{ marginBottom: "10px" }}>
            <input placeholder="column-name" style={{ marginRight: "8px" }} value={column} onInput={(ev) => {
                const copy = [...names]
                copy[i] = ev.currentTarget.value
                while (copy.length > 0 && copy.at(-1)! === "") {
                    copy.pop()
                }
                setNames(copy)
            }}></input>
            <Select style={{ marginRight: "8px" }} value={affinity.get(i) ?? "TEXT"} onChange={(value) => { setAffinity((d) => { d.set(i, value) }) }} options={{ "TEXT": {}, "NUMERIC": {}, "INTEGER": {}, "REAL": {}, "BLOB": {}, "ANY": {} }} />
            <Checkbox checked={primary.get(i) ?? false} onChange={(checked) => setPrimary((d) => { d.set(i, checked) })} text="PRIMARY KEY" />
            <Checkbox checked={autoIncrement.get(i) ?? false} onChange={(checked) => setAutoIncrement((d) => { d.set(i, checked) })} text="AUTOINCREMENT" />
            <Checkbox checked={unique.get(i) ?? false} onChange={(checked) => setUnique((d) => { d.set(i, checked) })} text="UNIQUE" />
            <Checkbox checked={notNull.get(i) ?? false} onChange={(checked) => setNotNull((d) => { d.set(i, checked) })} text="NOT NULL" />
        </div>
    })}</>
}

export const buildDispatch: DispatchBuilder<State> = (setState) => open = async () => { setState({ statement: statement, strict: true, tableConstraints: "", tableName: "", withoutRowId: false }) }

export const Title: TitleComponent<State> = (props) =>
    <> <input placeholder="table-name" value={props.state.tableName} onChange={(ev) => { props.setState({ ...props.state, tableName: ev.currentTarget.value }) }}></input>(...)
        <Checkbox checked={props.state.withoutRowId} onChange={(checked) => { props.setState({ ...props.state, withoutRowId: checked }) }} style={{ marginLeft: "8px" }} text="WITHOUT ROWID" />
        <Checkbox checked={props.state.strict} onChange={(checked) => { props.setState({ ...props.state, strict: checked }) }} text="STRICT" />
    </>

export const Editor: EditorComponent<State> = (props) => {
    const createTableColumnSchema = useRef<string>("")
    return <pre style={{ paddingTop: "15px" }}>
        <TableColumnSchemaEditor schema={createTableColumnSchema} />
        <textarea autocomplete="off" style={{ marginTop: "15px", width: "100%", height: "20vh", resize: "none" }} placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={props.state.tableConstraints} onChange={(ev) => { props.setState({ ...props.state, tableConstraints: ev.currentTarget.value }) }}></textarea><br></br>
        <Commit onClick={() => props.commit(`CREATE TABLE ${escapeSQLIdentifier(props.state.tableName)} (${createTableColumnSchema.current}${props.state.tableConstraints.trim() !== "" ? (props.state.tableConstraints.trim().startsWith(",") ? props.state.tableConstraints : ", " + props.state.tableConstraints) : ""})${props.state.strict ? " STRICT" : ""}${props.state.withoutRowId ? " WITHOUT ROWID" : ""}`, [])} />
    </pre>
}
