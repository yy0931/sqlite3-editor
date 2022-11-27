import { useEffect, useLayoutEffect, Ref, useState, useRef } from "preact/hooks"
import zustand from "zustand"
import produce, { enableMapSet } from "immer"
import type { JSXInternal } from "preact/src/jsx"
import { SQLite3Value, TableInfo, TableListItem } from "./sql"
import { blob2hex, escapeSQLIdentifier, type2color, useMainStore } from "./main"
import { Select } from "./components"
import { renderValue, unsafeEscapeValue, useTableStore } from "./table"

enableMapSet()

type State =
    | {
        tableName: string
    } & (
        | {
            statement: "ALTER TABLE"
            statement2: "RENAME TO" | "RENAME COLUMN" | "ADD COLUMN" | "DROP COLUMN"
            oldColumnName: string
            columnDef: ColumnDef
            newTableName: string
            newColumnName: string
        }
        | {
            statement: "DELETE"
            record: Record<string, SQLite3Value>
            constraintChoices: readonly (readonly string[])[]
            selectedConstraint: number
            row: number
        }
        | {
            statement: "DROP TABLE"
        }
        | {
            statement: "DROP VIEW"
        }
        | {
            statement: "INSERT"
            tableInfo: TableInfo
            textareaValues: string[]
            blobValues: (Uint8Array | null)[]
            dataTypes: EditorDataType[]
        }
        | {
            statement: "UPDATE"
            column: string
            record: Record<string, SQLite3Value>
            textareaValue: string
            blobValue: Uint8Array | null
            type: EditorDataType
            constraintChoices: readonly (readonly string[])[]
            row: number
            selectedConstraint: number
            isTextareaDirty: boolean
        }
    )
    | { tableName: string | undefined } & (
        | {
            statement: "CREATE TABLE"
            newTableName: string
            withoutRowId: boolean
            strict: boolean
            tableConstraints: string
            columnDefs: ColumnDef[]
        }
        | {
            statement: "custom"
            query: string
        }
    )

export const useEditorStore = zustand<State & {
    alterTable: (tableName: string, column: string | undefined) => void
    createTable: (tableName: string | undefined) => void
    delete_: (tableName: string, record: Record<string, SQLite3Value>, row: number) => Promise<void>
    dropTable: (tableName: string) => void
    dropView: (tableName: string) => void
    insert: (tableName: string) => Promise<void>
    custom: (tableName: string | undefined) => void
    update: (tableName: string, column: string, record: Record<string, SQLite3Value>, row: number) => Promise<void>
    switchTable: (tableName: string | undefined) => Promise<void>
    commit: (query: string, params: SQLite3Value[], opts: OnWriteOptions) => Promise<void>
    commitUpdate: () => Promise<void>
}>()((setPartial, get) => {
    const set = (state: State) => { setPartial(state) }
    return {
        statement: "CREATE TABLE",
        strict: true,
        tableConstraints: "",
        newTableName: "",
        withoutRowId: false,
        columnDefs: [],
        tableName: undefined,

        alterTable: (tableName: string, column: string | undefined) => {
            set({
                statement: "ALTER TABLE",
                tableName,
                statement2: column ? "RENAME COLUMN" : "RENAME TO",
                oldColumnName: column ?? "",
                columnDef: { name: "", affinity: "TEXT", autoIncrement: false, notNull: false, primary: false, unique: false },
                newColumnName: column ?? "",
                newTableName: tableName,
            })
        },
        createTable: (tableName: string | undefined) => {
            set({ statement: "CREATE TABLE", strict: true, newTableName: "", tableConstraints: "", withoutRowId: false, columnDefs: [], tableName })
        },
        delete_: async (tableName: string, record: Record<string, SQLite3Value>, row: number) => {
            set({
                statement: "DELETE",
                tableName,
                record,
                constraintChoices: ("rowid" in record ? [["rowid"]] : [])
                    .concat((await useMainStore.getState().sql.listUniqueConstraints(tableName)).sort((a, b) => +b.primary - +a.primary)
                        .map(({ columns }) => columns)
                        .filter((columns) => columns.every((column) => record[column] !== null))),
                selectedConstraint: 0,
                row,
            })
        },
        dropTable: (tableName: string) => {
            set({ statement: "DROP TABLE", tableName })
        },
        dropView: (tableName: string) => {
            set({ statement: "DROP VIEW", tableName })
        },
        insert: async (tableName: string) => {
            const tableInfo = await useMainStore.getState().sql.getTableInfo(tableName)
            set({
                statement: "INSERT",
                tableName, tableInfo,
                textareaValues: tableInfo.map(() => ""),
                blobValues: tableInfo.map(() => null),
                dataTypes: tableInfo.map(({ type }) => {
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
                }),
            })
        },
        custom: (tableName: string | undefined) => { set({ statement: "custom", query: "", tableName }) },
        update: async (tableName: string, column: string, record: Record<string, SQLite3Value>, row: number) => {
            const value = record[column]
            const constraintChoices = ("rowid" in record ? [["rowid"]] : [])
                .concat((await useMainStore.getState().sql.listUniqueConstraints(tableName)).sort((a, b) => +b.primary - +a.primary)
                    .map(({ columns }) => columns)
                    .filter((columns) => columns.every((column) => record[column] !== null)))
            if (constraintChoices.length === 0) { return }
            let type: EditorDataType
            if (value === null) {
                const columnAffinity = (await useMainStore.getState().sql.getTableInfo(tableName)).find(({ name }) => name === column)?.type.toUpperCase() ?? "ANY"
                // https://www.sqlite.org/datatype3.html#determination_of_column_affinity
                if (columnAffinity.includes("INT") || columnAffinity.includes("REAL") || columnAffinity.includes("FLOR") || columnAffinity.includes("DOUB")) {
                    type = "number"
                } else if (columnAffinity.includes("CHAR") || columnAffinity.includes("CLOB") || columnAffinity.includes("TEXT")) {
                    type = "string"
                } else if (columnAffinity.includes("BLOB")) { // or columnAffinity === ""
                    type = "blob"
                } else {
                    type = "null"
                }
            } else if (value instanceof Uint8Array) {
                type = "blob"
            } else if (typeof value === "number" || typeof value === "bigint") {
                type = "number"
            } else {
                type = "string"
            }
            set({
                statement: "UPDATE", tableName, column, record, constraintChoices, row, type,
                selectedConstraint: 0,
                textareaValue: value === null || value instanceof Uint8Array ? "" : (value + ""),
                blobValue: value instanceof Uint8Array ? value : null,
                isTextareaDirty: false,
            })
        },
        switchTable: async (tableName: string | undefined) => {
            const state = get()
            if (tableName === undefined) {
                state.createTable(tableName)
                return
            }
            switch (state.statement) {
                case "INSERT": await state.insert(tableName); break
                case "DROP TABLE": state.dropTable(tableName); break
                case "DROP VIEW": state.dropView(tableName); break
                case "ALTER TABLE": state.alterTable(tableName, undefined); break
                case "DELETE": case "UPDATE": await state.insert(tableName); break
                case "CREATE TABLE": case "custom": setPartial({ tableName }); break
                default: {
                    const _: never = state
                }
            }
        },
        commit: async (query: string, params: SQLite3Value[], opts: OnWriteOptions) => {
            await useMainStore.getState().sql.query(query, params, "w+")
            await useMainStore.getState().reload(opts)
            const state = get()
            await state.switchTable(state.tableName)  // clear inputs
        },
        commitUpdate: async () => {
            // <textarea> replaces \r\n with \n
            const state = get()
            if (state.statement !== "UPDATE" || !state.isTextareaDirty) { return }
            const columns = state.constraintChoices[state.selectedConstraint]!
            await state.commit(`UPDATE ${escapeSQLIdentifier(state.tableName)} SET ${escapeSQLIdentifier(state.column)} = ? WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`, [parseTextareaValue(state.textareaValue, state.blobValue, state.type), ...columns.map((column) => state.record[column] as SQLite3Value)], {})
        }
    }
})

export type OnWriteOptions = {
    refreshTableList?: true
    selectTable?: string
    /** refreshTableList and selectTable should be undefined */
    scrollToBottom?: true
}

export const Editor = (props: { tableList: TableListItem[] }) => {
    const state = useEditorStore()

    useLayoutEffect(() => {
        if (state.statement !== "UPDATE") { return }
        let unmount = () => { }
        const mount = () => {
            unmount()
            if (state.type === "number" || state.type === "string") {
                const textarea = document.createElement("textarea")
                textarea.style.color = type2color(state.type)

                const unsubscribe = useEditorStore.subscribe((state) => {
                    if (state.statement !== "UPDATE") { return }
                    textarea.style.color = type2color(state.type)
                })

                unmount = () => {
                    unsubscribe()
                    useTableStore.setState({ input: null })
                }

                textarea.classList.add("single-click")

                textarea.value = state.textareaValue
                textarea.addEventListener("input", () => {
                    textarea.classList.remove("single-click")
                    useEditorStore.setState({ textareaValue: textarea.value, isTextareaDirty: true })
                })
                textarea.addEventListener("click", () => {
                    textarea.classList.remove("single-click")
                })

                useTableStore.setState({
                    input: {
                        draftValue: renderValue(parseTextareaValue(state.textareaValue, state.blobValue, state.type)),
                        textarea,
                    }
                })
            } else {
                unmount = () => {
                    useTableStore.setState({ input: null })
                }
                useTableStore.setState({
                    input: {
                        draftValue: renderValue(parseTextareaValue(state.textareaValue, state.blobValue, state.type)),
                        textarea: null,
                    }
                })
            }
        }
        mount()
        useEditorStore.subscribe((state, prevState) => {
            if (state.statement !== "UPDATE" || prevState.statement !== "UPDATE" || state.type === prevState.type) { return }
            mount()
        })
        return unmount
    }, state.statement === "UPDATE" ? [state.row, state.column] : [])

    useLayoutEffect(() => {
        if (state.statement !== "UPDATE") { return }
        const input = useTableStore.getState().input
        if (input === null) { return }
        if (input.textarea !== null) {
            input.textarea.value = state.textareaValue
        }
        useTableStore.setState({ input: { ...input, draftValue: renderValue(parseTextareaValue(state.textareaValue, state.blobValue, state.type)) } })
    }, state.statement === "UPDATE" ? [state.textareaValue, state.blobValue, state.type] : [undefined, undefined, undefined])

    const { type } = props.tableList.find(({ name }) => name === state.tableName) ?? {}

    let header: JSXInternal.Element
    let editor: JSXInternal.Element

    switch (state.statement) {
        case "ALTER TABLE": {
            header = <>
                {escapeSQLIdentifier(state.tableName)} <Select value={state.statement2} onChange={(value) => useEditorStore.setState({ statement2: value })} options={{
                    "RENAME TO": {},
                    "RENAME COLUMN": {},
                    "DROP COLUMN": {},
                    "ADD COLUMN": {},
                }} />{" "}
                {state.statement2 === "RENAME TO" && <input placeholder="table-name" value={state.newTableName} onInput={(ev) => useEditorStore.setState({ newTableName: ev.currentTarget.value })} />}
                {(state.statement2 === "RENAME COLUMN" || state.statement2 === "DROP COLUMN") && <input placeholder="column-name" value={state.oldColumnName} onInput={(ev) => useEditorStore.setState({ oldColumnName: ev.currentTarget.value })} />}
                {state.statement2 === "RENAME COLUMN" && <>{" TO "}<input placeholder="column-name" value={state.newColumnName} onInput={(ev) => useEditorStore.setState({ newColumnName: ev.currentTarget.value })} /></>}
            </>
            editor = <>
                {state.statement2 === "ADD COLUMN" && <ColumnDefEditor value={state.columnDef} onChange={(columnDef) => useEditorStore.setState({ columnDef })} />}
                <Commit disabled={
                    state.statement2 === "RENAME TO" ? state.newTableName === "" :
                        state.statement2 === "RENAME COLUMN" ? state.oldColumnName === "" || state.newColumnName === "" :
                            state.statement2 === "DROP COLUMN" ? state.oldColumnName === "" :
                                state.statement2 === "ADD COLUMN" ? state.columnDef.name === "" :
                                    false
                } style={{ marginBottom: "10px" }} onClick={() => {
                    let query = `ALTER TABLE ${escapeSQLIdentifier(state.tableName)} ${state.statement2} `
                    switch (state.statement2) {
                        case "RENAME TO": query += escapeSQLIdentifier(state.newTableName); break
                        case "RENAME COLUMN": query += `${escapeSQLIdentifier(state.oldColumnName)} TO ${escapeSQLIdentifier(state.newColumnName)}`; break
                        case "DROP COLUMN": query += escapeSQLIdentifier(state.oldColumnName); break
                        case "ADD COLUMN": query += `${printColumnDef(state.columnDef)}`; break
                    }
                    state.commit(query, [], { refreshTableList: true, selectTable: state.statement2 === "RENAME TO" ? state.newTableName : undefined })
                }} />
            </>
            break
        }
        case "CREATE TABLE": {
            header = <>
                <input placeholder="table-name" value={state.newTableName} onInput={(ev) => useEditorStore.setState({ newTableName: ev.currentTarget.value })}></input>(...)
                <Checkbox checked={state.withoutRowId} onChange={(checked) => useEditorStore.setState({ withoutRowId: checked })} style={{ marginLeft: "8px" }} text="WITHOUT ROWID" />
                <Checkbox checked={state.strict} onChange={(checked) => useEditorStore.setState({ strict: checked })} text="STRICT" />
            </>
            editor = <>
                <MultiColumnDefEditor value={state.columnDefs} onChange={(columnDefs) => useEditorStore.setState({ columnDefs })} />
                <textarea autocomplete="off" style={{ marginTop: "10px", height: "20vh" }} placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={state.tableConstraints} onInput={(ev) => { useEditorStore.setState({ tableConstraints: ev.currentTarget.value }) }}></textarea>
                <Commit disabled={state.tableName === "" || state.columnDefs.length === 0} style={{ marginTop: "10px", marginBottom: "10px" }} onClick={() => {
                    state.commit(`CREATE TABLE ${escapeSQLIdentifier(state.newTableName)} (${state.columnDefs.map(printColumnDef).join(", ")}${state.tableConstraints.trim() !== "" ? (state.tableConstraints.trim().startsWith(",") ? state.tableConstraints : ", " + state.tableConstraints) : ""})${state.strict ? " STRICT" : ""}${state.withoutRowId ? " WITHOUT ROWID" : ""}`, [], { refreshTableList: true, selectTable: state.tableName })
                }} />
            </>
            break
        }
        case "DELETE": {
            const columns = state.constraintChoices[state.selectedConstraint]!
            header = <>
                FROM {escapeSQLIdentifier(state.tableName)} WHERE <select value={state.selectedConstraint} onInput={(ev) => { useEditorStore.setState({ selectedConstraint: +ev.currentTarget.value }) }}>{
                    state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(state.record[column] as SQLite3Value)}`).join(" AND ")}</option>)
                }</select>
            </>
            editor = <>
                <Commit style={{ marginBottom: "10px" }} onClick={() => {
                    state.commit(`DELETE FROM ${escapeSQLIdentifier(state.tableName)} WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`, [...columns.map((column) => state.record[column] as SQLite3Value)], {})
                }} />
            </>
            break
        }
        case "DROP TABLE": {
            header = <>{escapeSQLIdentifier(state.tableName)}</>
            editor = <><Commit style={{ marginBottom: "10px" }} onClick={() => state.commit(`DROP TABLE ${escapeSQLIdentifier(state.tableName)}`, [], { refreshTableList: true })} /></>
            break
        }
        case "DROP VIEW": {
            header = <>{escapeSQLIdentifier(state.tableName)}</>
            editor = <><Commit style={{ marginBottom: "10px" }} onClick={() => state.commit(`DROP VIEW ${escapeSQLIdentifier(state.tableName)}`, [], { refreshTableList: true })} /></>
            break
        }
        case "INSERT": {
            const query = `INTO ${escapeSQLIdentifier(state.tableName)} (${state.tableInfo.map(({ name }) => name).map(escapeSQLIdentifier).join(", ")}) VALUES (${state.tableInfo.map(() => "?").join(", ")})`
            header = <>{query}</>
            editor = <>
                <ul>
                    {state.tableInfo.map(({ name }, i) => {
                        return <li>
                            <div style={{ marginRight: "1em" }}>{name}</div>
                            <DataEditor
                                type={state.dataTypes[i]!}
                                rows={1}
                                style={{ width: "100%", resize: "vertical", display: "block", color: type2color(state.dataTypes[i]!) }}
                                textareaValue={state.textareaValues[i]!}
                                onTextareaValueChange={(value) => { useEditorStore.setState({ textareaValues: produce(state.textareaValues, (d) => { d[i] = value }) }) }}
                                blobValue={state.blobValues[i]!}
                                onBlobValueChange={(value) => { useEditorStore.setState({ blobValues: produce(state.blobValues, (d) => { d[i] = value }) }) }}
                                tabIndex={0} />
                            {"AS "}<DataTypeInput value={state.dataTypes[i]!} onChange={(value) => { useEditorStore.setState({ dataTypes: produce(state.dataTypes, (d) => { d[i] = value }) }) }} />
                        </li>
                    })}
                </ul>
                <Commit style={{ marginTop: "10px", marginBottom: "10px" }} onClick={() => {
                    state.commit(`INSERT ${query}`, state.textareaValues.map((value, i) => parseTextareaValue(value, state.blobValues[i]!, state.dataTypes[i]!)), { scrollToBottom: true })
                }} />
            </>
            break
        }
        case "UPDATE": {
            header = <>
                {escapeSQLIdentifier(state.tableName)} SET {escapeSQLIdentifier(state.column)} = ? WHERE <select value={state.selectedConstraint} onChange={(ev) => { useEditorStore.setState({ selectedConstraint: +ev.currentTarget.value }) }}>{
                    state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(state.record[column] as SQLite3Value)}`).join(" AND ")}</option>)
                }</select>
            </>
            editor = <>
                <DataEditor
                    rows={5}
                    type={state.type}
                    textareaValue={state.textareaValue}
                    onTextareaValueChange={(value) => useEditorStore.setState({ textareaValue: value, isTextareaDirty: true })}
                    blobValue={state.blobValue}
                    onBlobValueChange={(value) => useEditorStore.setState({ blobValue: value })}
                />
                {"AS "}
                <DataTypeInput value={state.type} onChange={(value) => useEditorStore.setState({ type: value })} />
                <Commit style={{ marginTop: "10px", marginBottom: "10px" }} onClick={state.commitUpdate} />
            </>
            break
        }
        case "custom": {
            header = <></>
            editor = <>
                <textarea autocomplete="off" style={{ marginTop: "15px", height: "20vh" }} placeholder={"CREATE TABLE table1(column1 INTEGER)"} value={state.query} onInput={(ev) => { useEditorStore.setState({ query: ev.currentTarget.value }) }}></textarea>
                <Commit style={{ marginBottom: "10px" }} onClick={() => state.commit(state.query, [], { refreshTableList: true })} />
            </>
            break
        }
        default: {
            const _: never = state
            throw new Error()
        }
    }

    return <>
        <h2>
            <Select value={state.statement} style={{ paddingLeft: "15px", paddingRight: "15px" }} className="primary" onChange={async (value) => {
                switch (value) {
                    case "ALTER TABLE": state.alterTable(state.tableName!, undefined); break
                    case "CREATE TABLE": state.createTable(state.tableName); break
                    case "DELETE": throw new Error()
                    case "DROP TABLE": state.dropTable(state.tableName!); break
                    case "DROP VIEW": state.dropView(state.tableName!); break
                    case "INSERT": state.insert(state.tableName!); break
                    case "UPDATE": throw new Error()
                    case "custom": state.custom(state.tableName); break
                    default: const _: never = value
                }
            }} options={{
                "ALTER TABLE": { disabled: type !== "table" && type !== "virtual", disabledReason: "Select a table or a virtual table." },
                "CREATE TABLE": {},
                DELETE: { disabled: true, disabledReason: "Click a row number." },
                "DROP TABLE": { disabled: type !== "table", disabledReason: "Select a table." },
                "DROP VIEW": { disabled: type !== "view", disabledReason: "Select a view." },
                INSERT: { disabled: type !== "table" && type !== "virtual", disabledReason: "Select a table or a virtual table." },
                UPDATE: { disabled: true, disabledReason: "Click a cell." },
                custom: {},
            }} />
            {" "}
            {header}
        </h2>
        <div>
            {editor}
        </div>
    </>
}

const DataTypeInput = (props: { value: EditorDataType, onChange: (value: EditorDataType) => void }) =>
    <Select value={props.value} onChange={props.onChange} tabIndex={-1} options={{ string: { text: "TEXT" }, number: { text: "NUMERIC" }, null: { text: "NULL" }, blob: { text: "BLOB" } }} />

type EditorDataType = "string" | "number" | "null" | "blob"

const DataEditor = (props: { rows?: number, style?: JSXInternal.CSSProperties, ref?: Ref<HTMLTextAreaElement & HTMLInputElement>, type: EditorDataType, textareaValue: string, onTextareaValueChange: (value: string) => void, blobValue: Uint8Array | null, onBlobValueChange: (value: Uint8Array) => void, tabIndex?: number }) => {
    const [filename, setFilename] = useState("")
    if (props.type === "blob") {
        return <div>
            <input value={"x'" + blob2hex(props.blobValue ?? new Uint8Array(), 8) + "'"} disabled={true} style={{ marginRight: "10px" }} />
            <input value={filename} placeholder={"tmp.dat"} onInput={(ev) => setFilename(ev.currentTarget.value)} />
            <input type="button" value="Import" onClick={() => useMainStore.getState().sql.import(filename).then((data) => props.onBlobValueChange(data))} disabled={filename === ""} />
            <input type="button" value="Export" onClick={() => useMainStore.getState().sql.export(filename, props.blobValue ?? new Uint8Array())} disabled={filename === "" || props.blobValue === null} />
        </div>
    }
    if (props.type === "string") {
        return <textarea
            ref={props.ref}
            rows={props.rows}
            autocomplete="off"
            style={{ color: type2color(props.type), resize: props.type === "string" ? "vertical" : "none", ...props.style }}
            value={props.textareaValue}
            onInput={(ev) => props.onTextareaValueChange(ev.currentTarget.value)}
            tabIndex={props.tabIndex} />
    }
    return <input
        ref={props.ref}
        autocomplete="off"
        style={{ color: type2color(props.type), display: "block" }}
        value={props.type === "null" ? "NULL" : props.textareaValue}
        onInput={(ev) => props.onTextareaValueChange(ev.currentTarget.value)}
        disabled={props.type === "null"}
        tabIndex={props.tabIndex} />
}

const parseTextareaValue = (value: string, blobValue: Uint8Array | null, type: EditorDataType): SQLite3Value => {
    if (type === "null") {
        return null
    } else if (type === "number") {
        if (/^[+\-]?\d+$/.test(value.trim()) && -(2n ** 64n / 2n) <= BigInt(value) && BigInt(value) <= 2n ** 64n / 2n - 1n) {
            return BigInt(value)  // i64
        } else {
            return +value
        }
    } else if (type === "blob") {
        return blobValue ?? new Uint8Array()
    } else {
        return value
    }
}

const Commit = (props: { disabled?: boolean, onClick: () => void, style?: JSXInternal.CSSProperties }) => {
    useEffect(() => {
        const handler = (ev: KeyboardEvent) => {
            if (ev.ctrlKey && ev.code === "KeyS") {
                props.onClick()
                ev.preventDefault()
            }
        }
        window.addEventListener("keydown", handler)
        return () => { window.removeEventListener("keydown", handler) }
    }, [props.onClick])
    return <input disabled={props.disabled} type="button" value="Commit" style={{ display: "block", ...props.style }} className={"primary"} onClick={props.onClick} title="Ctrl+S"></input>
}

const Checkbox = (props: { style?: JSXInternal.CSSProperties, checked: boolean, onChange: (value: boolean) => void, text: string, tabIndex?: number }) =>
    <label style={{ marginRight: "8px", ...props.style }}><input type="checkbox" checked={props.checked} onChange={(ev) => props.onChange(ev.currentTarget.checked)} tabIndex={props.tabIndex}></input> {props.text}</label>

type ColumnDef = {
    name: string
    affinity: "TEXT" | "NUMERIC" | "INTEGER" | "REAL" | "BLOB" | "ANY"
    primary: boolean
    autoIncrement: boolean
    unique: boolean
    notNull: boolean
}

const ColumnDefEditor = (props: { columnNameOnly?: boolean, value: ColumnDef, onChange: (columnDef: ColumnDef) => void }) => {
    return <>
        <input tabIndex={0} placeholder="column-name" style={{ marginRight: "8px" }} value={props.value.name} onInput={(ev) => { props.onChange({ ...props.value, name: ev.currentTarget.value }) }}></input>
        {!props.columnNameOnly && <>
            <Select tabIndex={0} style={{ marginRight: "8px" }} value={props.value.affinity} onChange={(value) => props.onChange({ ...props.value, affinity: value })} options={{ "TEXT": {}, "NUMERIC": {}, "INTEGER": {}, "REAL": {}, "BLOB": {}, "ANY": {} }} />
            <Checkbox tabIndex={-1} checked={props.value.primary} onChange={(checked) => props.onChange({ ...props.value, primary: checked })} text="PRIMARY KEY" />
            <Checkbox tabIndex={-1} checked={props.value.autoIncrement} onChange={(checked) => props.onChange({ ...props.value, autoIncrement: checked })} text="AUTOINCREMENT" />
            <Checkbox tabIndex={-1} checked={props.value.unique} onChange={(checked) => props.onChange({ ...props.value, unique: checked })} text="UNIQUE" />
            <Checkbox tabIndex={-1} checked={props.value.notNull} onChange={(checked) => props.onChange({ ...props.value, notNull: checked })} text="NOT NULL" />
        </>}
    </>
}

const printColumnDef = (def: ColumnDef) =>
    `${escapeSQLIdentifier(def.name)} ${def.affinity}${def.primary ? " PRIMARY KEY" : ""}${def.autoIncrement ? " AUTOINCREMENT" : ""}${def.unique ? " UNIQUE" : ""}${def.notNull ? " NOT NULL" : ""}`


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
