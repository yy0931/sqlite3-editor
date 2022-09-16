import { useState, useRef } from "preact/hooks"
import { useImmer } from "use-immer"
import { escapeSQLIdentifier, sql } from "../main"
import { Checkbox, Commit, Select } from "./components"

export type State = Readonly<{
    statement: "CREATE TABLE"
    tableName: string
    withoutRowId: boolean
    strict: boolean
    tableConstraints: string
}>

export let open: () => Promise<void>

const TableColumnSchemaEditor = (props: { schema: preact.RefObject<string> }) => {
    const [names, setNames] = useState<string[]>([])
    const [affinity, setAffinity] = useImmer(new Map<number, string>())
    const [primary, setPrimary] = useImmer(new Map<number, boolean>())
    const [unique, setUnique] = useImmer(new Map<number, boolean>())
    const [notNull, setNotNull] = useImmer(new Map<number, boolean>())

    props.schema.current = names.map((name, i) => `${escapeSQLIdentifier(name)} ${affinity.get(i) ?? "TEXT"}${primary.get(i) ? " PRIMARY" : ""}${unique.get(i) ? " UNIQUE" : ""}${notNull.get(i) ? " NOT NULL" : ""}`).join(", ")

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
            <Checkbox checked={primary.get(i) ?? false} onChange={(checked) => setPrimary((d) => { d.set(i, checked) })} text="PRIMARY" />
            <Checkbox checked={unique.get(i) ?? false} onChange={(checked) => setUnique((d) => { d.set(i, checked) })} text="UNIQUE" />
            <Checkbox checked={notNull.get(i) ?? false} onChange={(checked) => setNotNull((d) => { d.set(i, checked) })} text="NOT NULL" />
        </div>
    })}</>
}

export const init = (setState: (newState: State) => void) => {
    open = async () => { setState({ statement: "CREATE TABLE", strict: true, tableConstraints: "", tableName: "", withoutRowId: false }) }
}

export const Title = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    return <> <input placeholder="table-name" value={state.tableName} onChange={(ev) => { setState({ ...state, tableName: ev.currentTarget.value }) }}></input>(...)
        <Checkbox checked={state.withoutRowId} onChange={(checked) => { setState({ ...state, withoutRowId: checked }) }} style={{ marginLeft: "8px" }} text="WITHOUT ROWID" />
        <Checkbox checked={state.strict} onChange={(checked) => { setState({ ...state, strict: checked }) }} text="STRICT" />
    </>
}

export const Editor = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    const createTableColumnSchema = useRef<string>("")
    return <pre style={{ paddingTop: "15px" }}>
        <TableColumnSchemaEditor schema={createTableColumnSchema} />
        <textarea autocomplete="off" style={{ marginTop: "15px", width: "100%", height: "20vh", resize: "none" }} placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={state.tableConstraints} onChange={(ev) => { setState({ ...state, tableConstraints: ev.currentTarget.value }) }}></textarea><br></br>
        <Commit onClick={() => sql(`CREATE TABLE ${escapeSQLIdentifier(state.tableName)} (${createTableColumnSchema.current}${state.tableConstraints.trim() !== "" ? (state.tableConstraints.trim().startsWith(",") ? state.tableConstraints : ", " + state.tableConstraints) : ""})${state.strict ? " STRICT" : ""}${state.withoutRowId ? " WITHOUT ROWID" : ""}`, [], "w+").then(() => refreshTable())} />
    </pre>
}
