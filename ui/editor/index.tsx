import { useState, useCallback, useEffect } from "preact/hooks"
import * as immer from "immer"
import type { JSXInternal } from "preact/src/jsx"

immer.enableMapSet()

import * as insert from "./insert"
import * as createTable from "./create_table"
import * as dropTable from "./drop_table"
import * as dropView from "./drop_view"
import * as update from "./update"
import * as delete_ from "./delete_"
import * as alterTable from "./alter_table"
import * as custom from "./custom"
import { Select } from "./components"
import SQLite3Client, { DataTypes, TableListItem } from "../sql"

const editors = [insert, createTable, dropTable, dropView, update, delete_, alterTable, custom]

export type State = (typeof editors[number])["state"]

export type OnWriteOptions = {
    refreshTableList?: true
    selectTable?: string
    /** refreshTableList and selectTable should be undefined */
    scrollToBottom?: true
}

export const Editor = (props: { tableName?: string, tableList: TableListItem[], onWrite: (opts: OnWriteOptions) => void, sql: SQLite3Client }) => {
    const [state, setState] = useState<State>({ statement: "CREATE TABLE", strict: true, tableConstraints: "", tableName: "", withoutRowId: false })
    const commit = useCallback((query: string, params: DataTypes[], opts: OnWriteOptions) => props.sql.query(query, params, "w+").then(() => props.onWrite(opts)), [props.onWrite])

    useEffect(() => {
        if (props.tableName === undefined) {
            createTable.open()
            return
        }
        switch (state.statement) {
            case "INSERT": case "DROP TABLE": case "DROP VIEW": case "ALTER TABLE": case "DELETE":
                editors.find(({ statement }) => statement === state.statement)?.open(props.tableName)
                break
            case "UPDATE":
                insert.open(props.tableName)
                break
            case "CREATE TABLE": case "custom":
                break
            default: {
                const _: never = state
            }
        }
    }, [props.tableName])

    document.querySelectorAll(".editing").forEach((el) => el.classList.remove("editing"))
    if (state?.statement === "UPDATE") {
        state.td.classList.add("editing")
    } else if (state.statement === "DELETE") {
        state.tr.classList.add("editing")
    }

    for (const { buildDispatch } of editors) { buildDispatch(setState, props.sql) }

    const { type } = props.tableList.find(({ name }) => name === props.tableName) ?? {}

    const statementSelect = <Select value={state.statement} style={{ paddingLeft: "15px", paddingRight: "15px" }} className="primary" onChange={async (value) => {
        try {
            editors.find(({ statement }) => statement === value)?.open(props.tableName)
        } catch (err) {
            console.error(err)
        }
    }} options={{
        INSERT: { disabled: type !== "table" && type !== "virtual", disabledReason: "Select a table or a virtual table." },
        "CREATE TABLE": {},
        "DROP TABLE": { disabled: type !== "table", disabledReason: "Select a table." },
        "DROP VIEW": { disabled: type !== "view", disabledReason: "Select a view." },
        "ALTER TABLE": { disabled: type !== "table" && type !== "virtual", disabledReason: "Select a table or a virtual table." },
        UPDATE: { disabled: true, disabledReason: "Click a cell." },
        DELETE: { disabled: true, disabledReason: "Click a row number." },
        custom: {},
    }} />

    const editor = editors.find(({ statement }) => statement === state.statement)
    if (!editor) { throw new Error() }
    return <editor.Editor
        statementSelect={statementSelect}
        // @ts-ignore
        state={state}
        setState={setState}
        commit={commit} />
}

export type DispatchBuilder<T> = (setState: (newState: T) => void, sql: SQLite3Client) => void
export type EditorComponent<T> = (props: { statementSelect: JSXInternal.Element, state: T, setState: (newState: T) => void, commit: (query: string, params: DataTypes[], opts: OnWriteOptions) => Promise<void> }) => JSXInternal.Element