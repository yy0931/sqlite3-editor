import { useEffect, useLayoutEffect, Ref, MutableRef, useState, useRef } from "preact/hooks"
import zustand from "zustand"
import produce from "immer"
import type { JSXInternal } from "preact/src/jsx"
import * as remote from "./remote"
import { useMainStore } from "./main"
import { Button, Select } from "./components"
import { blob2hex, escapeSQLIdentifier, renderValue, type2color, unsafeEscapeValue, useTableStore } from "./table"

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
            strict: boolean
        }
        | {
            statement: "DELETE"
            record: Record<string, remote.SQLite3Value>
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
            tableInfo: remote.TableInfo
            textareaValues: string[]
            blobValues: (Uint8Array | null)[]
            dataTypes: EditorDataType[]
        }
        | {
            statement: "UPDATE"
            column: string
            record: Record<string, remote.SQLite3Value>
            textareaValue: string
            blobValue: Uint8Array | null
            type: EditorDataType
            constraintChoices: readonly (readonly string[])[]
            /** The row number relative to mainState.paging.visibleAreaTop */
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
            statement: "Custom Query"
            query: string
        }
    )

const inferTypeFromInputAndColumnAffinity = (input: string, column: string): EditorDataType => {
    const columnAffinity = useTableStore.getState().tableInfo.find(({ name }) => name === column)?.type.toUpperCase() ?? "ANY"
    // https://www.sqlite.org/datatype3.html#determination_of_column_affinity
    if (columnAffinity.includes("INT") || columnAffinity.includes("REAL") || columnAffinity.includes("FLOR") || columnAffinity.includes("DOUB")) {
        return "number"
    } else if (columnAffinity.includes("CHAR") || columnAffinity.includes("CLOB") || columnAffinity.includes("TEXT")) {
        return "string"
    } else {
        return /^[+\-\d\.]/.test(input) ? "number" : "string"
    }
}

let unmountInput: (() => void) | null = null
const mountInput = () => {
    unmountInput?.()
    const state = useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    if (state.type === "number" || state.type === "string" || state.type === "null") {
        const textarea = document.createElement("textarea")
        textarea.style.resize = "none"
        textarea.style.color = type2color(state.type)
        const unsubscribe = useEditorStore.subscribe((state) => {
            if (state.statement === "UPDATE") {
                textarea.style.color = type2color(state.type)
            }
        })

        unmountInput = () => {
            unmountInput = null
            unsubscribe()
            useTableStore.setState({ input: null })
        }

        textarea.tabIndex = -1
        textarea.classList.add("single-click")

        textarea.value = state.textareaValue
        textarea.addEventListener("input", (ev) => {
            textarea.classList.remove("single-click")
            const state = useEditorStore.getState()
            if (state.statement !== "UPDATE") { return }
            if (state.type === "null") {
                useEditorStore.setState({ textareaValue: textarea.value, isTextareaDirty: true, type: inferTypeFromInputAndColumnAffinity(textarea.value, state.column) })
            } else {
                useEditorStore.setState({ textareaValue: textarea.value, isTextareaDirty: true })
            }
        })
        textarea.addEventListener("mousedown", () => {
            textarea.classList.remove("single-click")
        })

        useTableStore.setState({
            input: {
                draftValue: renderValue(parseTextareaValue(state.textareaValue, state.blobValue, state.type)),
                draftValueType: state.type,
                textarea,
            }
        })
    } else {
        unmountInput = () => {
            unmountInput = null
            useTableStore.setState({ input: null })
        }
        useTableStore.setState({
            input: {
                draftValue: renderValue(parseTextareaValue(state.textareaValue, state.blobValue, state.type)),
                draftValueType: state.type,
                textarea: null,
            }
        })
    }
}

export const useEditorStore = zustand<State & {
    alterTable: (tableName: string, column: string | undefined) => Promise<void>
    createTable: (tableName: string | undefined) => void
    delete_: (tableName: string, record: Record<string, remote.SQLite3Value>, row: number) => Promise<void>
    dropTable: (tableName: string) => void
    dropView: (tableName: string) => void
    insert: (tableName: string) => Promise<void>
    custom: (tableName: string | undefined) => void
    update: (tableName: string, column: string, row: number) => void
    switchTable: (tableName: string | undefined) => Promise<void>
    commit: (query: string, params: remote.SQLite3Value[], opts: OnWriteOptions, preserveEditorState?: true) => Promise<void>
    commitUpdate: (preserveEditorState?: true, explicit?: true) => Promise<void>
    clearInputs: () => Promise<void>
    cancel: () => Promise<void>
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

        alterTable: async (tableName: string, column: string | undefined) => {
            unmountInput?.()
            set({
                statement: "ALTER TABLE",
                tableName,
                statement2: column ? "RENAME COLUMN" : "RENAME TO",
                oldColumnName: column ?? "",
                columnDef: { name: "", affinity: "TEXT", autoIncrement: false, notNull: false, primary: false, unique: false, default: "" },
                newColumnName: column ?? "",
                newTableName: tableName,
                strict: !!(await remote.getTableList()).find(({ name }) => name === tableName)?.strict,
            })
        },
        createTable: (tableName: string | undefined) => {
            unmountInput?.()
            set({ statement: "CREATE TABLE", strict: true, newTableName: "", tableConstraints: "", withoutRowId: false, columnDefs: [], tableName })
        },
        delete_: async (tableName: string, record: Record<string, remote.SQLite3Value>, row: number) => {
            unmountInput?.()
            set({
                statement: "DELETE",
                tableName,
                record,
                constraintChoices: useTableStore.getState().getRecordSelectors(record),
                selectedConstraint: 0,
                row,
            })
        },
        dropTable: (tableName: string) => {
            unmountInput?.()
            set({ statement: "DROP TABLE", tableName })
        },
        dropView: (tableName: string) => {
            unmountInput?.()
            set({ statement: "DROP VIEW", tableName })
        },
        insert: async (tableName: string) => {
            unmountInput?.()
            const { tableInfo } = useTableStore.getState()
            set({
                statement: "INSERT",
                tableName, tableInfo,
                textareaValues: tableInfo.map(() => ""),
                blobValues: tableInfo.map(() => null),
                dataTypes: tableInfo.map(({ type, notnull, dflt_value }): EditorDataType => {
                    if (notnull && dflt_value === null) {
                        type = type.toLowerCase()
                        if (type === "real" || type === "int" || type === "integer") {
                            return "number"
                        } else if (type === "text") {
                            return "string"
                        } else if (type === "null" || type === "blob") {
                            return type
                        } else {
                            return "string"
                        }
                    } else {
                        return "default"
                    }
                }),
            })
        },
        custom: (tableName: string | undefined) => {
            unmountInput?.()
            set({ statement: "Custom Query", query: "", tableName })
        },
        update: (tableName: string, column: string, row: number) => {
            unmountInput?.()
            const record = useTableStore.getState().records[row]!
            const value = record[column]
            const constraintChoices = useTableStore.getState().getRecordSelectors(record)
            if (constraintChoices.length === 0) { return }
            let type: EditorDataType
            if (value === null) {
                type = "null"
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
            mountInput()
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
                case "ALTER TABLE": await state.alterTable(tableName, undefined); break
                case "DELETE": case "UPDATE": await state.insert(tableName); break
                case "CREATE TABLE": case "Custom Query":
                    if (state.tableName === undefined) {
                        await state.insert(tableName)
                    } else {
                        setPartial({ tableName })
                    }
                    break
                default: {
                    const _: never = state
                }
            }
        },
        commit: async (query: string, params: remote.SQLite3Value[], opts: OnWriteOptions, preserveEditorState?: true) => {
            await remote.query(query, params, "w+")
            await useMainStore.getState().reloadAllTables(opts)
            if (!preserveEditorState) { await get().clearInputs() }
        },
        clearInputs: async () => {
            const state = get()
            switch (state.statement) {
                case "INSERT": setPartial({ textareaValues: state.textareaValues.map(() => ""), blobValues: state.blobValues.map(() => null) }); break
                case "DROP TABLE": state.dropTable(state.tableName); break
                case "DROP VIEW": state.dropView(state.tableName); break
                case "ALTER TABLE": await state.alterTable(state.tableName, undefined); break
                case "DELETE": case "UPDATE": await state.insert(state.tableName); break
                case "CREATE TABLE": state.createTable(state.tableName); break
                case "Custom Query": state.custom(state.tableName); break
                default: {
                    const _: never = state
                }
            }
        },
        cancel: async () => {
            const state = get()
            if (state.tableName === undefined) {
                state.createTable(state.tableName)
            } else {
                await state.insert(state.tableName)
            }
        },
        commitUpdate: async (preserveEditorState?: true, explicit?: true) => {
            // <textarea> replaces \r\n with \n
            const state = get()
            if (state.statement !== "UPDATE" || (!explicit && !state.isTextareaDirty)) { return }
            setPartial({ isTextareaDirty: false })
            const columns = state.constraintChoices[state.selectedConstraint]!
            await state.commit(`UPDATE ${escapeSQLIdentifier(state.tableName)} SET ${escapeSQLIdentifier(state.column)} = ? WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`, [parseTextareaValue(state.textareaValue, state.blobValue, state.type), ...columns.map((column) => state.record[column] as remote.SQLite3Value)], {}, preserveEditorState)
        }
    }
})

export type OnWriteOptions = {
    refreshTableList?: true
    selectTable?: string
    /** refreshTableList and selectTable should be undefined */
    scrollToBottom?: true
}

export const Editor = (props: { tableList: remote.TableListItem[] }) => {
    const state = useEditorStore()

    useLayoutEffect(() => {
        if (state.statement !== "UPDATE") { return }
        const input = useTableStore.getState().input
        if (input === null) { return }
        if (input.textarea !== null) {
            input.textarea.value = state.textareaValue
        }
        useTableStore.setState({ input: { ...input, draftValue: renderValue(parseTextareaValue(state.textareaValue, state.blobValue, state.type)), draftValueType: state.type } })
    }, state.statement === "UPDATE" ? [state.textareaValue, state.blobValue, state.type] : [undefined, undefined, undefined])

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
                {state.statement2 === "ADD COLUMN" && <ColumnDefEditor value={state.columnDef} onChange={(columnDef) => useEditorStore.setState({ columnDef })} strict={state.strict} />}
                <div className="mt-2">
                    <Commit disabled={
                        state.statement2 === "RENAME TO" ? state.newTableName === "" :
                            state.statement2 === "RENAME COLUMN" ? state.oldColumnName === "" || state.newColumnName === "" :
                                state.statement2 === "DROP COLUMN" ? state.oldColumnName === "" :
                                    state.statement2 === "ADD COLUMN" ? state.columnDef.name === "" :
                                        false
                    } onClick={() => {
                        let query = `ALTER TABLE ${escapeSQLIdentifier(state.tableName)} ${state.statement2} `
                        switch (state.statement2) {
                            case "RENAME TO": query += escapeSQLIdentifier(state.newTableName); break
                            case "RENAME COLUMN": query += `${escapeSQLIdentifier(state.oldColumnName)} TO ${escapeSQLIdentifier(state.newColumnName)}`; break
                            case "DROP COLUMN": query += escapeSQLIdentifier(state.oldColumnName); break
                            case "ADD COLUMN": query += `${printColumnDef(state.columnDef)}`; break
                        }
                        state.commit(query, [], { refreshTableList: true, selectTable: state.statement2 === "RENAME TO" ? state.newTableName : undefined }).catch(console.error)
                    }} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "CREATE TABLE": {
            header = <>
                <input placeholder="table-name" value={state.newTableName} onInput={(ev) => useEditorStore.setState({ newTableName: ev.currentTarget.value })}></input>(...)
                <Checkbox checked={state.withoutRowId} onChange={(checked) => useEditorStore.setState({ withoutRowId: checked })} className="[margin-left:8px]" text="WITHOUT ROWID" />
                <Checkbox checked={state.strict} onChange={(checked) => useEditorStore.setState({ strict: checked })} text="STRICT" />
            </>
            editor = <>
                <MultiColumnDefEditor value={state.columnDefs} onChange={(columnDefs) => useEditorStore.setState({ columnDefs })} strict={state.strict} />
                <textarea autocomplete="off" className="[margin-top:10px] [height:20vh]" placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={state.tableConstraints} onInput={(ev) => { useEditorStore.setState({ tableConstraints: ev.currentTarget.value }) }}></textarea>
                <div className="mt-2">
                    <Commit disabled={!state.newTableName || state.columnDefs.length === 0} className="[margin-top:10px] [margin-bottom:10px]" onClick={() => {
                        state.commit(`CREATE TABLE ${escapeSQLIdentifier(state.newTableName)} (${state.columnDefs.map(printColumnDef).join(", ")}${state.tableConstraints.trim() !== "" ? (state.tableConstraints.trim().startsWith(",") ? state.tableConstraints : ", " + state.tableConstraints) : ""})${state.strict ? " STRICT" : ""}${state.withoutRowId ? " WITHOUT ROWID" : ""}`, [], { refreshTableList: true, selectTable: state.newTableName }).catch(console.error)
                    }} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "DELETE": {
            const columns = state.constraintChoices[state.selectedConstraint]!
            header = <>
                FROM {escapeSQLIdentifier(state.tableName)} WHERE <select value={state.selectedConstraint} onInput={(ev) => { useEditorStore.setState({ selectedConstraint: +ev.currentTarget.value }) }}>{
                    state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(state.record[column] as remote.SQLite3Value)}`).join(" AND ")}</option>)
                }</select>
            </>
            editor = <>
                <div>
                    <Commit onClick={() => {
                        state.commit(`DELETE FROM ${escapeSQLIdentifier(state.tableName)} WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`, [...columns.map((column) => state.record[column] as remote.SQLite3Value)], {}).catch(console.error)
                    }} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "DROP TABLE": {
            header = <>{escapeSQLIdentifier(state.tableName)}</>
            editor = <>
                <div>
                    <Commit onClick={() => state.commit(`DROP TABLE ${escapeSQLIdentifier(state.tableName)}`, [], { refreshTableList: true })} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "DROP VIEW": {
            header = <>{escapeSQLIdentifier(state.tableName)}</>
            editor = <>
                <div>
                    <Commit onClick={() => state.commit(`DROP VIEW ${escapeSQLIdentifier(state.tableName)}`, [], { refreshTableList: true })} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "INSERT": {
            const filterDefaults = <T extends unknown>(_: T, i: number) => state.dataTypes[i] !== "default"
            const buildQuery = () => {
                return state.dataTypes.every((d) => d === "default") ?
                    `INTO ${escapeSQLIdentifier(state.tableName)} DEFAULT VALUES` :
                    `INTO ${escapeSQLIdentifier(state.tableName)} (${state.tableInfo.filter(filterDefaults).map(({ name }) => name).map(escapeSQLIdentifier).join(", ")}) VALUES (${state.tableInfo.filter(filterDefaults).map(() => "?").join(", ")})`
            }
            header = <>{buildQuery()}</>
            editor = <>
                <ul className="list-none">
                    {state.tableInfo.map(({ name }, i) => {
                        return <li>
                            <div className="[margin-right:1em]">{name}</div>
                            <DataEditor
                                column={name}
                                type={state.dataTypes[i]!}
                                rows={2}
                                className="w-full resize-y block"
                                style={{ color: type2color(state.dataTypes[i]!) }}
                                textareaValue={state.textareaValues[i]!}
                                onTextareaValueChange={(value) => { useEditorStore.setState({ textareaValues: produce(state.textareaValues, (d) => { d[i] = value }) }) }}
                                blobValue={state.blobValues[i]!}
                                onBlobValueChange={(value) => { useEditorStore.setState({ blobValues: produce(state.blobValues, (d) => { d[i] = value }) }) }}
                                onTypeChange={(type) => { useEditorStore.setState({ dataTypes: produce(state.dataTypes, (d) => { d[i] = type }) }) }}
                                tabIndex={0} />
                            {"AS "}<DataTypeInput value={state.dataTypes[i]!} onChange={(value) => { useEditorStore.setState({ dataTypes: produce(state.dataTypes, (d) => { d[i] = value }) }) }} />
                        </li>
                    })}
                </ul>
                <Commit className="mt-2" onClick={() => {
                    state.commit(`INSERT ${buildQuery()}`, state.textareaValues.filter(filterDefaults).map((value, i) => parseTextareaValue(value, state.blobValues[i]!, state.dataTypes[i]!)), { scrollToBottom: true }).catch(console.error)
                }} />
            </>
            break
        }
        case "UPDATE": {
            header = <>
                {escapeSQLIdentifier(state.tableName)} SET {escapeSQLIdentifier(state.column)} = ? WHERE <select value={state.selectedConstraint} onChange={(ev) => { useEditorStore.setState({ selectedConstraint: +ev.currentTarget.value }) }}>{
                    state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(state.record[column] as remote.SQLite3Value)}`).join(" AND ")}</option>)
                }</select>
            </>
            editor = <>
                <DataEditor
                    column={state.column}
                    rows={5}
                    type={state.type}
                    textareaValue={state.textareaValue}
                    onTextareaValueChange={(value) => useEditorStore.setState({ textareaValue: value, isTextareaDirty: true })}
                    blobValue={state.blobValue}
                    onBlobValueChange={(value) => useEditorStore.setState({ blobValue: value })}
                    onTypeChange={(type) => { useEditorStore.setState({ type }) }}
                />
                {"AS "}
                <DataTypeInput value={state.type} onChange={(type) => { useEditorStore.setState({ type }); mountInput() }} />
                <div className="mt-2">
                    <Commit onClick={() => state.commitUpdate(undefined, true)} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "Custom Query": {
            header = <></>
            editor = <>
                <textarea autocomplete="off" className="mb-2 [height:20vh]" placeholder={"CREATE TABLE table1(column1 INTEGER)"} value={state.query} onInput={(ev) => { useEditorStore.setState({ query: ev.currentTarget.value }) }}></textarea>
                <div className="mt-2">
                    <Commit onClick={() => state.commit(state.query, [], { refreshTableList: true })} />
                    <Cancel />
                </div>
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
            <span className="[color:var(--button-primary-background)]">{state.statement}</span>
            {" "}
            {header}
        </h2>
        <div className="[padding-left:var(--page-padding)] [padding-right:var(--page-padding)]">
            {editor}
        </div>
    </>
}

const DataTypeInput = (props: { value: EditorDataType, onChange: (value: EditorDataType) => void }) => {
    const options = { string: "TEXT", number: "NUMERIC", null: "NULL", blob: "BLOB", default: "DEFAULT" }
    return <>{
        Object.entries(options)
            .map(([k, v]) => <Checkbox tabIndex={-1} text={v} checked={props.value === k} onChange={(value) => value ? props.onChange(k as keyof typeof options) : props.onChange("default")} />)
    }</>
}

type EditorDataType = "string" | "number" | "null" | "blob" | "default"

const DataEditor = (props: { column: string, rows?: number, style?: JSXInternal.CSSProperties, ref?: Ref<HTMLTextAreaElement & HTMLInputElement>, type: EditorDataType, textareaValue: string, onTextareaValueChange: (value: string) => void, blobValue: Uint8Array | null, onBlobValueChange: (value: Uint8Array) => void, tabIndex?: number, className?: string, onTypeChange: (type: EditorDataType) => void }) => {
    const [filename, setFilename] = useState("")
    if (props.type === "blob") {
        return <div>
            <input value={"x'" + blob2hex(props.blobValue ?? new Uint8Array(), 8) + "'"} disabled={true} className={"w-40 inline [margin-right:10px] " + (props.className ?? "")} />
            <input value={filename} placeholder={"tmp.dat"} onInput={(ev) => setFilename(ev.currentTarget.value)} />
            <Button onClick={() => remote.import_(filename).then((data) => props.onBlobValueChange(data))} disabled={filename === ""}>Import</Button>
            <Button onClick={() => remote.export_(filename, props.blobValue ?? new Uint8Array())} disabled={filename === "" || props.blobValue === null}>Export</Button>
        </div>
    }

    const ref = useRef() as Ref<HTMLTextAreaElement>
    useLayoutEffect(() => {
        ref.current!.style.height = ""  // Reset the height of the textarea
    }, [props.type])
    useEffect(() => {
        if (props.ref) {
            (props.ref as MutableRef<HTMLTextAreaElement>).current = ref.current!
        }
    }, [ref.current])

    const placeholder = ((): string => {
        switch (props.type) {
            case "default":
                const dflt_value = useTableStore.getState().tableInfo.find(({ name }) => name === props.column)?.dflt_value
                if (dflt_value === undefined || dflt_value === null) { return "NULL" }
                return dflt_value + ""
            case "null": return "NULL"
            case "string": return "<empty string>"
            case "number": return "0"
            default:
                const _: never = props.type
                throw new Error()
        }
    })()
    return <textarea
        placeholder={placeholder}
        ref={ref}
        rows={props.type === "string" ? props.rows : 1}
        autocomplete="off"
        style={{ color: type2color(props.type), resize: props.type === "string" ? "vertical" : "none", ...props.style }}
        className={`data-editor-${props.type} ` + (props.className ?? "")}
        value={props.type === "null" || props.type === "default" ? "" : props.textareaValue}
        onInput={(ev) => {
            if (props.type === "null" || props.type === "default") {
                props.onTypeChange(inferTypeFromInputAndColumnAffinity(ev.currentTarget.value, props.column))
            }
            props.onTextareaValueChange(ev.currentTarget.value)
        }}
        tabIndex={props.tabIndex} />
}

const parseTextareaValue = (value: string, blobValue: Uint8Array | null, type: EditorDataType): remote.SQLite3Value => {
    if (type === "null") {
        return null
    } else if (type === "number") {
        if (value.trim() === "") {
            return 0n
        } else if (/^[+\-]?\d+$/.test(value.trim()) && -(2n ** 64n / 2n) <= BigInt(value) && BigInt(value) <= 2n ** 64n / 2n - 1n) {
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

const Commit = (props: { disabled?: boolean, onClick: () => void, style?: JSXInternal.CSSProperties, className?: string }) => {
    useEffect(() => {
        const handler = (ev: KeyboardEvent) => {
            if (ev.ctrlKey && ev.code === "Enter") {
                props.onClick()
                ev.preventDefault()
            }
        }
        window.addEventListener("keydown", handler)
        return () => { window.removeEventListener("keydown", handler) }
    }, [props.onClick])
    return <Button disabled={props.disabled} style={props.style} className={"mb-2 " + (props.className ?? "")} onClick={props.onClick}>
        Commit<span className="opacity-60 ml-2 [font-size:70%]">Ctrl+Enter</span>
    </Button>
}

const Cancel = (props: { disabled?: boolean, style?: JSXInternal.CSSProperties, className?: string }) => {
    return <Button disabled={props.disabled} style={props.style} className={"mb-2 ml-2 [background-color:var(--dropdown-background)] [color:var(--dropdown-foreground)] hover:[background-color:#8e8e8e] " + (props.className ?? "")} onClick={() => { useEditorStore.getState().cancel().catch(console.error) }}>
        Cancel
    </Button>
}

const Checkbox = (props: { style?: JSXInternal.CSSProperties, checked: boolean, onChange: (value: boolean) => void, text: string, tabIndex?: number, className?: string }) =>
    <label className={"select-none mr-2 cursor-pointer " + (props.className ?? "")} tabIndex={0} style={{ borderBottom: "1px solid gray", color: props.checked ? "rgba(0, 0, 0)" : "rgba(0, 0, 0, 0.4)", ...props.style }} onClick={() => props.onChange(!props.checked)} onKeyDown={(ev) => { if (["Enter", "Space"].includes(ev.code)) { props.onChange(!props.checked) } }}>
        {props.text}
    </label>

type ColumnDef = {
    name: string
    affinity: "TEXT" | "NUMERIC" | "INTEGER" | "REAL" | "BLOB" | "ANY"
    primary: boolean
    autoIncrement: boolean
    unique: boolean
    notNull: boolean
    default: string
}

const ColumnDefEditor = (props: { columnNameOnly?: boolean, value: ColumnDef, onChange: (columnDef: ColumnDef) => void, strict: boolean }) => {
    return <>
        <input tabIndex={0} placeholder="column-name" className="[margin-right:8px]" value={props.value.name} onInput={(ev) => { props.onChange({ ...props.value, name: ev.currentTarget.value }) }}></input>
        {!props.columnNameOnly && <>
            <Select tabIndex={0} className="[margin-right:8px]" value={props.value.affinity} onChange={(value) => props.onChange({ ...props.value, affinity: value })} options={{ "TEXT": {}, "INTEGER": {}, "REAL": {}, "BLOB": {}, "ANY": { disabled: !props.strict, disabledReason: "STRICT tables only." }, "NUMERIC": { disabled: props.strict, disabledReason: "non-STRICT tables only." } }} />
            <Checkbox tabIndex={-1} checked={props.value.primary} onChange={(checked) => props.onChange({ ...props.value, primary: checked })} text="PRIMARY KEY" />
            <Checkbox tabIndex={-1} checked={props.value.autoIncrement} onChange={(checked) => props.onChange({ ...props.value, autoIncrement: checked })} text="AUTOINCREMENT" />
            <Checkbox tabIndex={-1} checked={props.value.unique} onChange={(checked) => props.onChange({ ...props.value, unique: checked })} text="UNIQUE" />
            <Checkbox tabIndex={-1} checked={props.value.notNull} onChange={(checked) => props.onChange({ ...props.value, notNull: checked })} text="NOT NULL" />
            <label className="[margin-right:8px]" style={{ color: props.value.default ? "rgba(0, 0, 0)" : "rgba(0, 0, 0, 0.4)" }}>DEFAULT</label><input placeholder="CURRENT_TIMESTAMP" value={props.value.default} onChange={(el) => props.onChange({ ...props.value, default: el.currentTarget.value })} />
        </>}
    </>
}

const printColumnDef = (def: ColumnDef) =>
    `${escapeSQLIdentifier(def.name)} ${def.affinity}${def.primary ? " PRIMARY KEY" : ""}${def.autoIncrement ? " AUTOINCREMENT" : ""}${def.unique ? " UNIQUE" : ""}${def.notNull ? " NOT NULL" : ""}${def.default ? ` DEFAULT (${def.default})` : ""}`  // TODO: don't enclose with () when def.default is a literal

const MultiColumnDefEditor = (props: { value: ColumnDef[], onChange: (value: ColumnDef[]) => void, strict: boolean }) => {
    const renderedColumnDefs = [...props.value]
    while (renderedColumnDefs.at(-1)?.name === "") { renderedColumnDefs.pop() }
    if (renderedColumnDefs.length === 0 || renderedColumnDefs.at(-1)!.name !== "") {
        renderedColumnDefs.push({ name: "", affinity: "TEXT", autoIncrement: false, notNull: false, primary: false, unique: false, default: "" })
    }

    return <ul className="list-none">{renderedColumnDefs.map((columnDef, i) =>
        <li key={i}>
            <ColumnDefEditor columnNameOnly={i === renderedColumnDefs.length - 1 && columnDef.name === ""} value={columnDef} onChange={(value) => {
                props.onChange(produce(renderedColumnDefs, (d) => {
                    d[i] = value
                    while (d.at(-1)?.name === "") { d.pop() }
                }))
            }} strict={props.strict} />
        </li>
    )}</ul>
}
