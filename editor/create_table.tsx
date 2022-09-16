import { useState, useRef } from "preact/hooks"
import { useImmer } from "use-immer"
import { escapeSQLIdentifier, sql } from "../main"

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
        return <div style={{ marginBottom: "10px" }}><input placeholder="column-name" style={{ marginRight: "8px" }} value={column} onInput={(ev) => {
            const copy = [...names]
            copy[i] = ev.currentTarget.value
            while (copy.length > 0 && copy.at(-1)! === "") {
                copy.pop()
            }
            setNames(copy)
        }}></input>
            <select title="column affinity" style={{ marginRight: "8px" }} value={affinity.get(i) ?? "TEXT"} onChange={(ev) => { setAffinity((d) => { d.set(i, ev.currentTarget.value) }) }}>
                <option>TEXT</option>
                <option>NUMERIC</option>
                <option>INTEGER</option>
                <option>REAL</option>
                <option>BLOB</option>
                <option>ANY</option>
            </select>
            <label style={{ marginRight: "8px" }}><input type="checkbox" checked={primary.get(i) ?? false} onChange={(ev) => setPrimary((d) => { d.set(i, ev.currentTarget.checked) })}></input> PRIMARY</label>
            <label style={{ marginRight: "8px" }}><input type="checkbox" checked={unique.get(i) ?? false} onChange={(ev) => setUnique((d) => { d.set(i, ev.currentTarget.checked) })}></input> UNIQUE</label>
            <label style={{ marginRight: "8px" }}><input type="checkbox" checked={notNull.get(i) ?? false} onChange={(ev) => setNotNull((d) => { d.set(i, ev.currentTarget.checked) })}></input> NOT NULL</label >
        </div >
    })}</>
}

export const init = (setState: (newState: State) => void) => {
    open = async () => { setState({ statement: "CREATE TABLE", strict: true, tableConstraints: "", tableName: "", withoutRowId: false }) }
    return <></>
}

export const Title = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    return <> <input placeholder="table-name" value={state.tableName} onChange={(ev) => { setState({ ...state, tableName: ev.currentTarget.value }) }}></input>(...)
        <label style={{ marginLeft: "8px", marginRight: "8px" }}><input type="checkbox" checked={state.withoutRowId} onChange={(ev) => { setState({ ...state, withoutRowId: ev.currentTarget.checked }) }}></input> WITHOUT ROWID</label>
        <label style={{ marginRight: "8px" }}><input type="checkbox" checked={state.strict} onChange={(ev) => { setState({ ...state, strict: ev.currentTarget.checked }) }}></input> STRICT</label>
    </>
}

export const Editor = ({ state, refreshTable, setState }: { state: State, refreshTable: () => void, setState: (newState: State) => void }) => {
    const createTableColumnSchema = useRef<string>("")
    return <pre style={{ paddingTop: "15px" }}>
        <TableColumnSchemaEditor schema={createTableColumnSchema} />
        <textarea autocomplete="off" style={{ marginTop: "15px", width: "100%", height: "20vh", resize: "none" }} placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={state.tableConstraints} onChange={(ev) => { setState({ ...state, tableConstraints: ev.currentTarget.value }) }}></textarea><br></br>
        <input type="button" value="Commit" style={{ display: "block", marginTop: "15px", fontSize: "125%", color: "white", background: "var(--accent-color)" }} onClick={() => {
            sql(`CREATE TABLE ${escapeSQLIdentifier(state.tableName)} (${createTableColumnSchema.current}${state.tableConstraints.trim() !== "" ? (state.tableConstraints.trim().startsWith(",") ? state.tableConstraints : ", " + state.tableConstraints) : ""})${state.strict ? " STRICT" : ""}${state.withoutRowId ? " WITHOUT ROWID" : ""}`, [], "w+")
                .then(() => refreshTable())
        }}></input>
    </pre>
}
