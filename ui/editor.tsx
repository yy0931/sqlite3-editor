import { useEffect, useLayoutEffect, MutableRef, useState, useRef, Ref } from "preact/hooks"
import produce from "immer"
import type { JSXInternal } from "preact/src/jsx"
import * as remote from "./remote"
import { Button, Checkbox, Highlight, Select } from "./components"
import { blob2hex, escapeSQLIdentifier, renderValue, type2color, unsafeEscapeValue, useTableStore } from "./table"
import { BigintMath, createStore } from "./util"

type State =
    // SQL statements that require a table name
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
        | {
            statement: "CREATE INDEX"
            unique: boolean
            indexName: string
            indexedColumns: string
            where: string
        }
    )

    // SQL statements that don't require a table name
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
            statement: "DROP INDEX"
            indexName: string
        }
        | {
            statement: "Custom Query"
            query: string
        }
    )

/** This function infers the data type of the input based on the column affinity of the given column.  */
const inferTypeFromInputAndColumnAffinity = (input: string, column: string): EditorDataType => {
    const columnAffinity = useTableStore.getState().tableInfo.find(({ name }) => name === column)?.type.toUpperCase() ?? "ANY"
    // https://www.sqlite.org/datatype3.html#determination_of_column_affinity
    if (columnAffinity.includes("INT") || columnAffinity.includes("REAL") || columnAffinity.includes("FLOR") || columnAffinity.includes("DOUB")) {
        // INTEGER or REAL
        return "number"
    } else if (columnAffinity.includes("CHAR") || columnAffinity.includes("CLOB") || columnAffinity.includes("TEXT")) {
        // TEXT
        return "string"
    } else {
        // NUMERIC or ANY
        return /^[+\-\d\.]/.test(input) ? "number" : "string"
    }
}

let unmountInput: (() => void) | null = null

/** Appends a `<textarea>` to a table cell. */
const mountInput = () => {
    unmountInput?.()
    const state = useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    if (state.type === "number" || state.type === "string" || state.type === "null") {
        const textarea = document.createElement("textarea")
        textarea.dataset.testid = "inplaceInput"
        textarea.spellcheck = false
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
                textarea: null,
            }
        })
    }
}

/** A store for the editor component. */
export const useEditorStore = createStore("useEditorStore", {
    statement: "CREATE TABLE",
    tableName: undefined,
    strict: true,
    tableConstraints: "",
    newTableName: "",
    withoutRowId: false,
    columnDefs: [],
} satisfies State as State, (setPartial, get) => {
    const set = (state: State) => { setPartial(state) }

    /** Switches the editor to the ALTER TABLE editor. */
    const alterTable = async (tableName: string, column: string | undefined) => {
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
    }

    /** Switches the editor to the CREATE TABLE editor. */
    const createTable = (tableName: string | undefined) => {
        unmountInput?.()
        set({ statement: "CREATE TABLE", strict: true, newTableName: "", tableConstraints: "", withoutRowId: false, columnDefs: [], tableName })
    }

    /** Switches the editor to the DELETE editor. */
    const delete_ = async (tableName: string, record: Record<string, remote.SQLite3Value>, row: number) => {
        unmountInput?.()
        set({
            statement: "DELETE",
            tableName,
            record,
            constraintChoices: useTableStore.getState().getRecordSelectors(record),
            selectedConstraint: 0,
            row,
        })
    }

    /** Switches the editor to the DROP TABLE editor. */
    const dropTable = (tableName: string) => {
        unmountInput?.()
        set({ statement: "DROP TABLE", tableName })
    }

    /** Switches the editor to the DROP VIEW editor. */
    const dropView = (tableName: string) => {
        unmountInput?.()
        set({ statement: "DROP VIEW", tableName })
    }

    /** Switches the editor to the INSERT editor. */
    const insert = async (tableName: string) => {
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
    }

    /** Switches the editor to the custom query editor. */
    const custom = (tableName: string | undefined) => {
        unmountInput?.()
        set({ statement: "Custom Query", query: "", tableName })
    }

    /** Switches the editor to the UPDATE editor. */
    const update = (tableName: string, column: string, row: number) => {
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
    }

    /** Switches the editor to the CREATE INDEX editor. */
    const createIndex = (tableName: string) => {
        unmountInput?.()
        set({ statement: "CREATE INDEX", tableName, unique: false, indexName: "", indexedColumns: "", where: "" })
    }

    /** Switches the editor to the DROP INDEX editor. */
    const dropIndex = (tableName: string | undefined, indexName: string) => {
        unmountInput?.()
        set({ statement: "DROP INDEX", indexName, tableName })
    }

    /** Discards changes made in the editor. Keeps the SQL statement except for UPDATE. isDirty() should become false after calling this function. */
    const discardChanges = async () => {
        const state = get()
        switch (state.statement) {
            case "INSERT": setPartial({ textareaValues: state.textareaValues.map(() => ""), blobValues: state.blobValues.map(() => null) }); break
            case "DROP TABLE": dropTable(state.tableName); break
            case "DROP VIEW": dropView(state.tableName); break
            case "ALTER TABLE": await alterTable(state.tableName, undefined); break
            case "DELETE": case "UPDATE": await insert(state.tableName); break
            case "CREATE TABLE": createTable(state.tableName); break
            case "Custom Query": custom(state.tableName); break
            case "CREATE INDEX": createIndex(state.tableName); break
            case "DROP INDEX": dropIndex(state.tableName, state.indexName); break
        }
    }

    const isDirty = () => {
        const s = get()
        switch (s.statement) {
            case "UPDATE": return s.isTextareaDirty
            case "INSERT": return s.textareaValues.some((value) => value !== "") || s.blobValues.some((value) => value !== null)
            case "DROP TABLE": return false
            case "DROP VIEW": return false
            case "ALTER TABLE": return false
            case "DELETE": return false
            case "CREATE TABLE": return s.newTableName !== "" || s.tableConstraints !== "" || s.columnDefs.length !== 0
            case "Custom Query": return s.query !== ""
            case "CREATE INDEX": return s.indexName !== "" || s.indexedColumns !== "" || s.where !== ""
            case "DROP INDEX": return false
        }
    }

    /** Commits changes. */
    const commit = async (query: string, params: readonly remote.SQLite3Value[], opts: OnWriteOptions, preserveEditorState?: true) => {
        // Prevent loops
        if (get().statement === "UPDATE") {
            setPartial({ isTextareaDirty: false })
        } else {
            await discardChanges()
        }

        // Query
        await remote.query(query, params, "w+")

        if (opts.reload === "allTables") {
            await useTableStore.getState().reloadAllTables(opts.selectTable)
        } else {
            await useTableStore.getState().reloadTable(true, true)
            if (opts.scrollToBottom) {
                let state = useTableStore.getState()
                await state.setPaging({ visibleAreaTop: BigintMath.max(state.paging.numRecords - state.paging.visibleAreaSize + 1n, 0n) })
                state = useTableStore.getState()
            }
        }
        if (!preserveEditorState) { await discardChanges() }
    }

    const buildQuery = (): [string, remote.SQLite3Value[], OnWriteOptions] => {
        const s = get()
        switch (s.statement) {
            case "UPDATE": {
                const columns = s.constraintChoices[s.selectedConstraint]!
                return [
                    `UPDATE ${escapeSQLIdentifier(s.tableName)} SET ${escapeSQLIdentifier(s.column)} = ? WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`,
                    [parseTextareaValue(s.textareaValue, s.blobValue, s.type), ...columns.map((column) => s.record[column] as remote.SQLite3Value)],
                    { reload: "currentTable" },
                ]
            }
            case "INSERT": {
                const filterDefaults = <T extends unknown>(_: T, i: number) => s.dataTypes[i] !== "default"
                return [
                    s.dataTypes.every((d) => d === "default") ?
                        `INSERT INTO ${escapeSQLIdentifier(s.tableName)} DEFAULT VALUES` :
                        `INSERT INTO ${escapeSQLIdentifier(s.tableName)} (${s.tableInfo.filter(filterDefaults).map(({ name }) => name).map(escapeSQLIdentifier).join(", ")}) VALUES (${s.tableInfo.filter(filterDefaults).map(() => "?").join(", ")})`,
                    s.textareaValues.filter(filterDefaults).map((value, i) => parseTextareaValue(value, s.blobValues[i]!, s.dataTypes[i]!)),
                    { reload: "currentTable", scrollToBottom: true },
                ]
            }
            case "DROP TABLE": return [`DROP TABLE ${escapeSQLIdentifier(s.tableName)}`, [], { reload: "allTables" }]
            case "DROP VIEW": return [`DROP VIEW ${escapeSQLIdentifier(s.tableName)}`, [], { reload: "allTables" }]
            case "ALTER TABLE": {
                let query = `ALTER TABLE ${escapeSQLIdentifier(s.tableName)} ${s.statement2} `
                switch (s.statement2) {
                    case "RENAME TO": query += escapeSQLIdentifier(s.newTableName); break
                    case "RENAME COLUMN": query += `${escapeSQLIdentifier(s.oldColumnName)} TO ${escapeSQLIdentifier(s.newColumnName)}`; break
                    case "DROP COLUMN": query += escapeSQLIdentifier(s.oldColumnName); break
                    case "ADD COLUMN": query += `${printColumnDef(s.columnDef)}`; break
                }
                return [query, [], { reload: "allTables", selectTable: s.statement2 === "RENAME TO" ? s.newTableName : undefined }]
            }
            case "DELETE": {
                const columns = s.constraintChoices[s.selectedConstraint]!
                return [
                    `DELETE FROM ${escapeSQLIdentifier(s.tableName)} WHERE ${columns.map((column) => `${column} = ?`).join(" AND ")}`,
                    [...columns.map((column) => s.record[column] as remote.SQLite3Value)],
                    { reload: "currentTable" },
                ]
            }
            case "CREATE TABLE": return [
                `CREATE TABLE ${escapeSQLIdentifier(s.newTableName)} (${s.columnDefs.map(printColumnDef).join(", ")}${s.tableConstraints.trim() !== "" ? (s.tableConstraints.trim().startsWith(",") ? s.tableConstraints : ", " + s.tableConstraints) : ""})${s.strict ? " STRICT" : ""}${s.withoutRowId ? " WITHOUT ROWID" : ""}`,
                [],
                { reload: "allTables", selectTable: s.newTableName },
            ]
            case "Custom Query": return [s.query, [], { reload: "allTables" }]
            case "CREATE INDEX": return [
                `CREATE${s.unique ? " UNIQUE" : ""} INDEX ${escapeSQLIdentifier(s.indexName)} ON ${escapeSQLIdentifier(s.tableName)} (${s.indexedColumns})${s.where ? ` WHERE ${s.where}` : ""}`,
                [],
                { reload: "allTables" },
            ]
            case "DROP INDEX": return [`DROP INDEX ${escapeSQLIdentifier(s.indexName)}`, [], { reload: "allTables" }]
        }
    }

    return {
        alterTable,
        createTable,
        delete_,
        dropTable,
        dropView,
        insert,
        custom,
        update,
        createIndex,
        dropIndex,
        commit,
        discardChanges,
        buildQuery,
        isDirty,
        switchTable: async (tableName: string | undefined) => {
            const state = get()
            if (useTableStore.getState().useCustomViewerQuery) {
                custom(tableName)
                return
            } else if (tableName === undefined) {
                createTable(tableName)
                return
            }
            switch (state.statement) {
                case "INSERT": await insert(tableName); break
                case "DROP TABLE": dropTable(tableName); break
                case "DROP VIEW": dropView(tableName); break
                case "ALTER TABLE": await alterTable(tableName, undefined); break
                case "DELETE": case "UPDATE": await insert(tableName); break
                case "DROP INDEX": await insert(tableName); break
                case "CREATE INDEX": await createIndex(tableName); break
                case "CREATE TABLE": case "Custom Query":
                    if (state.tableName === undefined) {
                        await insert(tableName)
                    } else {
                        setPartial({ tableName })
                    }
                    break
            }
        },
        cancel: async () => {
            const state = get()
            if (useTableStore.getState().useCustomViewerQuery) {
                custom(state.tableName)
            } else if (state.tableName === undefined) {
                createTable(state.tableName)
            } else {
                await insert(state.tableName)
            }
        },
        /**
         * If the editor is dirty, this function asks the user whether the changes should be committed.
         * @param keepUpdateEditor - Keep it open when the UPDATE editor is open.
         * @returns false if canceled, true otherwise
         */
        beforeUnmount: async (keepUpdateEditor?: true): Promise<boolean> => {
            if (!isDirty()) { return true }
            const returnValue = await useTableStore.getState().confirm()
            if (returnValue === "cancel") { return false }
            if (returnValue === "discard changes") {
                if (!(get().statement === "UPDATE" && keepUpdateEditor)) { await discardChanges() }
                return true
            }

            // Build a query before discarding changes
            const query = buildQuery()

            await commit(...query, keepUpdateEditor)
            return true
        }
    }
})

export type OnWriteOptions = {
    reload: "allTables"
    /** Select the table if it exists */
    selectTable?: string
} | {
    reload: "currentTable"
    scrollToBottom?: true
}

export const Editor = () => {
    const state = useEditorStore()

    useLayoutEffect(() => {
        if (state.statement !== "UPDATE") { return }
        const input = useTableStore.getState().input
        if (input === null) { return }
        if (input.textarea !== null) {
            input.textarea.value = state.textareaValue
        }
        useTableStore.setState({ input: { ...input, draftValue: renderValue(parseTextareaValue(state.textareaValue, state.blobValue, state.type)) } })
    }, state.statement === "UPDATE" ? [state.textareaValue, state.blobValue, state.type] : [undefined, undefined, undefined])

    let header: JSXInternal.Element
    let editor: JSXInternal.Element

    switch (state.statement) {
        case "ALTER TABLE": {
            header = <>
                <Highlight data-testid="ALTER TABLE">ALTER TABLE </Highlight>
                {escapeSQLIdentifier(state.tableName)} <Select value={state.statement2} onChange={(value) => useEditorStore.setState({ statement2: value })} options={{
                    "RENAME TO": {},
                    "RENAME COLUMN": {},
                    "DROP COLUMN": {},
                    "ADD COLUMN": {},
                }} />{" "}
                {state.statement2 === "RENAME TO" && <input placeholder="table-name" value={state.newTableName} onInput={(ev) => useEditorStore.setState({ newTableName: ev.currentTarget.value })} data-testid="alter-table-rename-to-new-table-name" />}
                {(state.statement2 === "RENAME COLUMN" || state.statement2 === "DROP COLUMN") && <input placeholder="column-name" value={state.oldColumnName} onInput={(ev) => useEditorStore.setState({ oldColumnName: ev.currentTarget.value })} />}
                {state.statement2 === "RENAME COLUMN" && <>{" TO "}<input placeholder="column-name" value={state.newColumnName} onInput={(ev) => useEditorStore.setState({ newColumnName: ev.currentTarget.value })} /></>}
            </>
            const query = state.buildQuery()
            editor = <>
                {state.statement2 === "ADD COLUMN" && <ColumnDefEditor value={state.columnDef} onChange={(columnDef) => useEditorStore.setState({ columnDef })} strict={state.strict} />}
                <div class="mt-2">
                    <Commit disabled={
                        state.statement2 === "RENAME TO" ? state.newTableName === "" :
                            state.statement2 === "RENAME COLUMN" ? state.oldColumnName === "" || state.newColumnName === "" :
                                state.statement2 === "DROP COLUMN" ? state.oldColumnName === "" :
                                    state.statement2 === "ADD COLUMN" ? state.columnDef.name === "" :
                                        false
                    } query={query[0]} onClick={() => { state.commit(...query).catch(console.error) }} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "CREATE TABLE": {
            header = <>
                <Highlight data-testid="CREATE TABLE">CREATE TABLE </Highlight>
                <input placeholder="table-name" value={state.newTableName} onInput={(ev) => useEditorStore.setState({ newTableName: ev.currentTarget.value })} data-testid="CREATE TABLE > table-name"></input>(...)
                <Checkbox checked={state.withoutRowId} onChange={(checked) => useEditorStore.setState({ withoutRowId: checked })} class="ml-[8px]" text="WITHOUT ROWID" />
                <Checkbox checked={state.strict} onChange={(checked) => useEditorStore.setState({ strict: checked })} text="STRICT" />
            </>
            const query = state.buildQuery()
            editor = <>
                <MultiColumnDefEditor value={state.columnDefs} onChange={(columnDefs) => useEditorStore.setState({ columnDefs })} strict={state.strict} />
                <textarea autocomplete="off" spellcheck={false} class="mt-[10px] h-[20vh]" placeholder={"FOREIGN KEY(column-name) REFERENCES table-name(column-name)"} value={state.tableConstraints} onInput={(ev) => { useEditorStore.setState({ tableConstraints: ev.currentTarget.value }) }}></textarea>
                <div class="mt-2">
                    <Commit query={query[0]} disabled={!state.newTableName || state.columnDefs.length === 0} class="mt-[10px] mb-[10px]" onClick={() => {
                        state.commit(...query).catch(console.error)
                    }} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "DELETE": {
            header = <>
                <Highlight data-testid="DELETE FROM">DELETE FROM </Highlight>
                {escapeSQLIdentifier(state.tableName)} WHERE <select class="pl-1" value={state.selectedConstraint} onInput={(ev) => { useEditorStore.setState({ selectedConstraint: +ev.currentTarget.value }) }}>{
                    state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(state.record[column] as remote.SQLite3Value)}`).join(" AND ")}</option>)
                }</select>
            </>
            const query = state.buildQuery()
            editor = <>
                <div>
                    <Commit query={query[0]} onClick={() => { state.commit(...query).catch(console.error) }} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "DROP TABLE": {
            header = <>
                <Highlight data-testid="DROP TABLE">DROP TABLE </Highlight>
                {escapeSQLIdentifier(state.tableName)}
            </>
            const query = state.buildQuery()
            editor = <>
                <div>
                    <Commit query={query[0]} onClick={() => state.commit(...query)} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "DROP VIEW": {
            header = <>
                <Highlight data-testid="DROP VIEW">DROP VIEW </Highlight>
                {escapeSQLIdentifier(state.tableName)}
            </>
            const query = state.buildQuery()
            editor = <>
                <div>
                    <Commit query={query[0]} onClick={() => state.commit(...query)} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "INSERT": {
            const query = state.buildQuery()
            header = <>
                <Highlight data-testid="INSERT">INSERT INTO </Highlight>
                {query[0].slice("INSERT INTO ".length)}
            </>
            editor = <>
                <ul class="list-none">
                    {state.tableInfo.map(({ name }, i) => {
                        return <li id={`insert-column${i + 1}`} data-testid={`insert-column ${i + 1}`}>
                            <div class="mr-[1em]">{name}</div>
                            <DataEditor
                                column={name}
                                type={state.dataTypes[i]!}
                                rows={2}
                                class="w-full resize-y block"
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
                <Commit class="mt-2" query={query[0]} onClick={() => {
                    state.commit(...query).catch(console.error)
                }} />
            </>
            break
        }
        case "UPDATE": {
            header = <>
                <Highlight data-testid="UPDATE">UPDATE </Highlight>
                {escapeSQLIdentifier(state.tableName)} SET {escapeSQLIdentifier(state.column)} = ? WHERE <select value={state.selectedConstraint} class="pl-1" onChange={(ev) => { useEditorStore.setState({ selectedConstraint: +ev.currentTarget.value }) }}>{
                    state.constraintChoices.map((columns, i) => <option value={i}>{columns.map((column) => `${column} = ${unsafeEscapeValue(state.record[column] as remote.SQLite3Value)}`).join(" AND ")}</option>)
                }</select>
            </>
            const query = state.buildQuery()
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
                <div class="mt-2">
                    <Commit query={query[0]} onClick={() => { state.commit(...query).catch(console.error) }} />
                    <Cancel />
                </div>
            </>
            break
        }
        case "CREATE INDEX": {
            header = <>
                <Highlight data-testid="CREATE INDEX">
                    {"CREATE "}
                    <Checkbox text="UNIQUE" checked={state.unique} onChange={(unique) => useEditorStore.setState({ unique })} />
                    {"INDEX "}
                </Highlight>
                <input placeholder="index-name" value={state.indexName} onInput={(ev) => useEditorStore.setState({ indexName: ev.currentTarget.value })}></input>
                {" ON "}
                {escapeSQLIdentifier(state.tableName)}
                {" ("}
                <input placeholder="column1, column2" value={state.indexedColumns} onInput={(ev) => useEditorStore.setState({ indexedColumns: ev.currentTarget.value })}></input>
                {") "}
                <label class="mr-[8px]" style={{ color: state.where !== "" ? "rgba(0, 0, 0)" : "rgba(0, 0, 0, 0.4)" }}>WHERE</label>
                <input placeholder="expr" value={state.where} onInput={(ev) => useEditorStore.setState({ where: ev.currentTarget.value })}></input>
            </>
            const query = state.buildQuery()
            editor = <>
                <Commit query={query[0]} onClick={() => state.commit(...query).catch(console.error)} />
                <Cancel />
            </>
            break
        }
        case "DROP INDEX": {
            header = <>
                <Highlight data-testid="DROP INDEX">DROP INDEX </Highlight>
                <span class="[color:var(--button-primary-background)]">DROP INDEX</span> {escapeSQLIdentifier(state.indexName)}
            </>
            const query = state.buildQuery()
            editor = <>
                <Commit query={query[0]} onClick={() => state.commit(...query).catch(console.error)} />
                <Cancel />
            </>
            break
        }
        case "Custom Query": {
            header = <><Highlight data-testid="Custom Query">Custom Query </Highlight></>
            const query = state.buildQuery()
            editor = <>
                <textarea autocomplete="off" spellcheck={false} class="mb-2 h-[20vh]" placeholder={"CREATE TABLE table1(column1 INTEGER)"} value={state.query} onInput={(ev) => { useEditorStore.setState({ query: ev.currentTarget.value }) }}></textarea>
                <div class="mt-2">
                    <Commit query={query[0]} onClick={() => state.commit(...query)} />
                    <Cancel />
                </div>
            </>
            break
        }
    }

    return <div id="editor">
        <h2>
            {header}
        </h2>
        <div class="pl-[var(--page-padding)] pr-[var(--page-padding)]">
            {editor}
        </div>
    </div>
}

const DataTypeInput = (props: { value: EditorDataType, onChange: (value: EditorDataType) => void }) => {
    return <>
        <Checkbox tabIndex={-1} text="TEXT" checked={props.value === "string"} onChange={(value) => value ? props.onChange("string") : props.onChange("default")} />
        <Checkbox tabIndex={-1} text="NUMERIC" checked={props.value === "number"} onChange={(value) => value ? props.onChange("number") : props.onChange("default")} />
        <Checkbox tabIndex={-1} text="BLOB" checked={props.value === "blob"} onChange={(value) => value ? props.onChange("blob") : props.onChange("default")} />
        <Checkbox tabIndex={-1} text="NULL" checked={props.value === "null"} onChange={(value) => value ? props.onChange("null") : props.onChange("default")} />
        <Checkbox tabIndex={-1} text="DEFAULT" checked={props.value === "default"} onChange={(value) => value ? props.onChange("default") : props.onChange("default")} />
    </>
}

type EditorDataType = "string" | "number" | "null" | "blob" | "default"

let editorHeight = new Map<string, string>()

/** Renders a textarea for the given data type `props.type`. */
const DataEditor = (props: { column: string, rows?: number, style?: JSXInternal.CSSProperties, type: EditorDataType, textareaValue: string, onTextareaValueChange: (value: string) => void, blobValue: Uint8Array | null, onBlobValueChange: (value: Uint8Array) => void, tabIndex?: number, class?: string, onTypeChange: (type: EditorDataType) => void }) => {
    const [filename, setFilename] = useState("")

    const ref = useRef<HTMLTextAreaElement>(null)

    useLayoutEffect(() => {
        if (!ref.current) { return }
        const textarea = ref.current
        if (props.type === "string") {
            textarea.style.height = editorHeight.get(props.column) ?? ""
        } else {
            textarea.style.height = ""  // Reset the height of the textarea
        }

        if (props.type !== "string") { return }
        const observer = new ResizeObserver((a) => {
            if (!textarea || !textarea.style.height) { return }
            console.log(props.column, textarea.style.height)
            editorHeight.set(props.column, textarea.style.height)
        })
        observer.observe(textarea)
        return () => { observer.disconnect() }
    }, [ref.current, props.type, props.column])

    // BLOB
    if (props.type === "blob") {
        return <div>
            <input value={"x'" + blob2hex(props.blobValue ?? new Uint8Array(), 8) + "'"} disabled={true} class={"w-40 inline mr-[10px] " + (props.class ?? "")} />
            <input value={filename} placeholder={"tmp.dat"} onInput={(ev) => setFilename(ev.currentTarget.value)} />
            <Button onClick={() => remote.import_(filename).then((data) => props.onBlobValueChange(data))} disabled={filename === ""}>Import</Button>
            <Button onClick={() => remote.export_(filename, props.blobValue ?? new Uint8Array())} disabled={filename === "" || props.blobValue === null}>Export</Button>
        </div>
    }

    const placeholder = ((): string => {
        switch (props.type) {
            case "default":
                const dflt_value = useTableStore.getState().tableInfo.find(({ name }) => name === props.column)?.dflt_value
                if (dflt_value === undefined || dflt_value === null) { return "NULL" }
                return dflt_value + ""
            case "null": return "NULL"
            case "string": return "<empty string>"
            case "number": return "0"
        }
    })()

    // TEXT, INTEGER, REAL, NULL
    return <textarea
        placeholder={placeholder}
        ref={ref}
        rows={props.type === "string" ? props.rows : 1}
        autocomplete="off"
        spellcheck={false}
        style={{
            color: type2color(props.type),
            resize: props.type === "string" ? "vertical" : "none",
            ...props.type === "string" ? { height: editorHeight.get(props.column) ?? "" } : { height: "" },
            ...props.style,
        }}
        class={`data-editor-${props.type} ` + (props.class ?? "")}
        value={props.type === "null" || props.type === "default" ? "" : props.textareaValue}
        onInput={(ev) => {
            if (props.type === "null" || props.type === "default") {
                props.onTypeChange(inferTypeFromInputAndColumnAffinity(ev.currentTarget.value, props.column))
            }
            props.onTextareaValueChange(ev.currentTarget.value)
        }}
        tabIndex={props.tabIndex} data-testid="editor-textarea" />
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

const Commit = (props: { disabled?: boolean, onClick: () => void, style?: JSXInternal.CSSProperties, class?: string, query: string }) => {
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
    return <Button disabled={props.disabled} style={props.style} class={"mb-2 " + (props.class ?? "")} onClick={props.onClick} title={props.query} data-testid="commit">
        Commit<span class="opacity-60 ml-2 [font-size:70%]">Ctrl+Enter</span>
    </Button>
}

const Cancel = (props: { disabled?: boolean, style?: JSXInternal.CSSProperties, class?: string }) => {
    return <Button disabled={props.disabled} style={props.style} class={"mb-2 ml-2 bg-[var(--dropdown-background)] [color:var(--dropdown-foreground)] hover:[background-color:#8e8e8e] " + (props.class ?? "")} onClick={() => { useEditorStore.getState().cancel().catch(console.error) }}>
        Cancel
    </Button>
}

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
        <input tabIndex={0} placeholder="column-name" class="mr-[8px]" value={props.value.name} onInput={(ev) => { props.onChange({ ...props.value, name: ev.currentTarget.value }) }} data-testid="column-name"></input>
        {!props.columnNameOnly && <>
            <Select tabIndex={0} class="mr-[8px]" value={props.value.affinity} onChange={(value) => props.onChange({ ...props.value, affinity: value })} options={{ "TEXT": {}, "INTEGER": {}, "REAL": {}, "BLOB": {}, "ANY": { disabled: !props.strict, disabledReason: "STRICT tables only." }, "NUMERIC": { disabled: props.strict, disabledReason: "non-STRICT tables only." } }} data-testid="column-datatype" />
            <Checkbox tabIndex={-1} checked={props.value.primary} onChange={(checked) => props.onChange({ ...props.value, primary: checked })} text="PRIMARY KEY" />
            <Checkbox tabIndex={-1} checked={props.value.autoIncrement} onChange={(checked) => props.onChange({ ...props.value, autoIncrement: checked })} text="AUTOINCREMENT" />
            <Checkbox tabIndex={-1} checked={props.value.unique} onChange={(checked) => props.onChange({ ...props.value, unique: checked })} text="UNIQUE" />
            <Checkbox tabIndex={-1} checked={props.value.notNull} onChange={(checked) => props.onChange({ ...props.value, notNull: checked })} text="NOT NULL" />
            <label class="mr-[8px]" style={{ color: props.value.default ? "rgba(0, 0, 0)" : "rgba(0, 0, 0, 0.4)" }}>DEFAULT</label><input placeholder="CURRENT_TIMESTAMP" value={props.value.default} onChange={(el) => props.onChange({ ...props.value, default: el.currentTarget.value })} />
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

    return <ul class="list-none">{renderedColumnDefs.map((columnDef, i) =>
        <li key={i} data-testid={`column ${i + 1}`}>
            <ColumnDefEditor columnNameOnly={i === renderedColumnDefs.length - 1 && columnDef.name === ""} value={columnDef} onChange={(value) => {
                props.onChange(produce(renderedColumnDefs, (d) => {
                    d[i] = value
                    while (d.at(-1)?.name === "") { d.pop() }
                }))
            }} strict={props.strict} />
        </li>
    )}</ul>
}
