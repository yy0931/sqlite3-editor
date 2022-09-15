import { useState, useRef, useEffect, Ref } from "preact/hooks"
import { blob2hex, DataTypes, escapeSQLIdentifier, getTableInfo, getTableList, listUniqueConstraints, sql, TableInfo, type2color, UniqueConstraints, unsafeEscapeValue } from "./main"
import produce, * as immer from "immer"
import { useImmer } from "use-immer"

immer.enableMapSet()

type State =
    | Readonly<{ statement: "INSERT", tableName: string, tableInfo: TableInfo, values: string[], dataTypes: EditorDataType[] }>
    | Readonly<{ statement: "UPDATE", tableName: string, column: string, record: Record<string, DataTypes>, textareaValue: string, type: EditorDataType, constraintChoices: readonly (readonly string[])[], selectedConstraint: number, td: HTMLElement }>
    | Readonly<{ statement: "CREATE TABLE", tableName: string, withoutRowId: boolean, strict: boolean, tableConstraints: string }>

export let insert: () => Promise<void>
export let update: (column: string, record: Record<string, DataTypes>, td: HTMLElement) => Promise<void>
export let createTable: () => Promise<void>

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

type EditorDataType = "string" | "number" | "null" | "blob"

const DataTypeInput = (props: { value: EditorDataType, onChange: (value: EditorDataType) => void }) => {
    return <select autocomplete="off" value={props.value} onChange={(ev) => { props.onChange(ev.currentTarget.value as any) }} tabIndex={-1}>
        <option value="string">TEXT</option>
        <option value="number">NUMERIC</option>
        <option value="null">NULL</option>
        <option value="blob">BLOB</option>
    </select>
}

const parseTextareaValue = (value: string, type: EditorDataType): DataTypes => {
    if (type === "null") {
        return null
    } else if (type === "number") {
        return +value
    } else if (type === "blob") {
        return Uint8Array.from(value.match(/.{1, 2}/g)?.map((byte) => parseInt(byte, 16)) ?? /* TODO: Show an error message*/[])
    } else {
        return value
    }
}

export const Editor = (props: { refreshTable: () => void }) => {
    const [state, setState] = useState<State | null>(null)
    const createTableColumnSchema = useRef<string>("")
    const autoFocusRef = useRef(null) as Ref<HTMLTextAreaElement>

    useEffect(() => {
        autoFocusRef.current?.focus()
    }, [state?.statement === "UPDATE" ? state.td : null])

    document.querySelectorAll(".editing").forEach((el) => el.classList.remove("editing"))
    if (state?.statement === "UPDATE") {
        state.td.classList.add("editing")
    }

    insert = async () => {
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
    update = async (column, record, td) => {
        const tableName = document.querySelector<HTMLSelectElement>("#tableSelect")!.value
        const value = record[column]
        const uniqueConstraints = await listUniqueConstraints(tableName)
        const withoutRowId = !!(await getTableList()).find(({ name }) => name === tableName)!.wr
        setState({
            statement: "UPDATE",
            tableName,
            column,
            record,
            textareaValue: value instanceof Uint8Array ? blob2hex(value) : (value + ""),
            type: value === null ? "null" : value instanceof Uint8Array ? "blob" : typeof value === "number" ? "number" : "string",
            constraintChoices: uniqueConstraints.sort((a, b) => +b.primary - +a.primary)
                .map(({ columns }) => columns)
                .concat(withoutRowId ? [] : [["rowid"]])
                .filter((columns) => columns.every((column) => record[column] !== null)),
            selectedConstraint: 0,
            td,
        })
    }
    createTable = async () => { setState({ statement: "CREATE TABLE", strict: true, tableConstraints: "", tableName: "", withoutRowId: false }) }

    if (state === null) { return <></> }

    let title: preact.ComponentChild
    let editor: preact.ComponentChild
    switch (state.statement) {
        case "INSERT": {
            const query = `INTO ${escapeSQLIdentifier(state.tableName)} (${state.tableInfo.map(({ name }) => name).map(escapeSQLIdentifier).join(", ")}) VALUES (${state.tableInfo.map(() => "?").join(", ")})`
            title = <> {query}</>
            editor = <pre style={{ paddingTop: "4px" }}>
                {state.tableInfo.map(({ name }, i) => {
                    return <><div style={{ marginTop: "10px", marginBottom: "2px" }}>{name}</div><textarea autocomplete="off" style={{ width: "100%", height: "25px", resize: "vertical", display: "block", color: type2color(state.dataTypes[i]!) }} value={state.values[i]!} onChange={(ev) => { setState(produce(state, (d) => { d.values[i] = ev.currentTarget.value })) }} tabIndex={0}></textarea> AS <DataTypeInput value={state.dataTypes[i]!} onChange={(value) => { setState(produce(state, (d) => { d.dataTypes[i] = value })) }} /></>
                })}
                <input type="button" value="Commit" style={{ display: "block", marginTop: "15px", fontSize: "125%", color: "white", background: "rgba(0, 0, 0, 0.678)" }} onClick={() => {
                    sql(`INSERT ${query}`, state.values.map((value, i) => parseTextareaValue(value, state.dataTypes[i]!)), "w+")
                        .then(() => props.refreshTable())
                }}></input>
            </pre>
            break
        } case "CREATE TABLE":
            title = <> <input placeholder="table-name" value={state.tableName} onChange={(ev) => { setState({ ...state, tableName: ev.currentTarget.value }) }}></input>(...)
                <label style={{ marginLeft: "8px", marginRight: "8px" }}><input type="checkbox" checked={state.withoutRowId} onChange={(ev) => { setState({ ...state, withoutRowId: ev.currentTarget.checked }) }}></input> WITHOUT ROWID</label>
                <label style={{ marginRight: "8px" }}><input type="checkbox" checked={state.strict} onChange={(ev) => { setState({ ...state, strict: ev.currentTarget.checked }) }}></input> STRICT</label>
            </>
            editor = <pre style={{ paddingTop: "15px" }}>
                <TableColumnSchemaEditor schema={createTableColumnSchema} />
                <textarea autocomplete="off" style={{ marginTop: "15px", width: "100%", height: "20vh", resize: "none" }} placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={state.tableConstraints} onChange={(ev) => { setState({ ...state, tableConstraints: ev.currentTarget.value }) }}></textarea><br></br>
                <input type="button" value="Commit" style={{ display: "block", marginTop: "15px", fontSize: "125%", color: "white", background: "rgba(0, 0, 0, 0.678)" }} onClick={() => {
                    sql(`CREATE TABLE ${escapeSQLIdentifier(state.tableName)} (${createTableColumnSchema.current}${state.tableConstraints.trim() !== "" ? (state.tableConstraints.trim().startsWith(",") ? state.tableConstraints : ", " + state.tableConstraints) : ""})${state.strict ? " STRICT" : ""}${state.withoutRowId ? " WITHOUT ROWID" : ""}`, [], "w+")
                        .then(() => props.refreshTable())
                }}></input>
            </pre>
            break
        case "UPDATE":
            title = <> {escapeSQLIdentifier(state.tableName)} SET {escapeSQLIdentifier(state.column)} = ? <select value={state.selectedConstraint} onChange={(ev) => { setState({ ...state, selectedConstraint: +ev.currentTarget.value }) }}>{
                state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `WHERE ${column} = ${unsafeEscapeValue(state.record[column])}`).join(" ")}</option>)
            }</select></>
            editor = <pre>
                <textarea ref={autoFocusRef} autocomplete="off" style={{ width: "100%", height: "20vh", resize: "none", color: type2color(state.type) }} value={state.textareaValue} onChange={(ev) => {
                    const columns = state.constraintChoices[state.selectedConstraint]!
                    sql(`UPDATE ${escapeSQLIdentifier(state.tableName)} SET ${escapeSQLIdentifier(state.column)} = ? ` + columns.map((column) => `WHERE ${column} = ?`).join(" "), [parseTextareaValue(ev.currentTarget.value, state.type), ...columns.map((column) => state.record[column] as DataTypes)], "w+")
                        .then(() => props.refreshTable())
                        .catch(console.error)
                    insert()
                }}></textarea>
                AS <DataTypeInput value={state.type} onChange={(value) => { setState({ ...state, type: value }) }} />
            </pre>
            break
        default: {
            const _: never = state
        }
    }
    return <>
        <h2>
            <pre>
                <select autocomplete="off" value={state.statement} style={{ color: "white", background: "rgba(0, 0, 0, 0.678)", paddingLeft: "15px", paddingRight: "15px" }} onChange={async (ev) => {
                    try {
                        const nextStatement = ev.currentTarget.value as State["statement"]
                        switch (nextStatement) {
                            case "INSERT": insert(); break
                            case "UPDATE": throw new Error()
                            case "CREATE TABLE": createTable(); break
                            default: { const _: never = nextStatement }
                        }
                    } catch (err) {
                        console.error(err)
                    }
                }}>
                    <option value="INSERT">INSERT</option>
                    <option value="CREATE TABLE">CREATE TABLE</option>
                    <option value="UPDATE" disabled title="Click a cell to change a cell value">UPDATE</option>
                </select>
                <span id="editorTitle">{title}</span>
            </pre>
        </h2>
        {editor}
    </>
}
