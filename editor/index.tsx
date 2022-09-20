import { useState, useCallback } from "preact/hooks"
import * as immer from "immer"
import type { JSXInternal } from "preact/src/jsx"

immer.enableMapSet()

import * as insert from "./insert"
import * as createTable from "./create_table"
import * as dropTable from "./drop_table"
import * as update from "./update"
import * as delete_ from "./delete_"
import * as alterTable from "./alter_table"
import { Select } from "./components"
import SQLite3Client, { DataTypes } from "../sql"

const editors = [insert, createTable, dropTable, update, delete_, alterTable]

export type State = (typeof editors[number])["state"]

type OnWriteOptions = { refreshTableList?: true, selectTable?: string }

export const Editor = (props: { tableName?: string, onWrite: (opts: OnWriteOptions) => void, sql: SQLite3Client }) => {
    const [state, setState] = useState<State>({ statement: "CREATE TABLE", strict: true, tableConstraints: "", tableName: "", withoutRowId: false })
    const commit = useCallback((query: string, params: DataTypes[], opts: OnWriteOptions) => props.sql.query(query, params, "w+").then(() => props.onWrite(opts)).catch(console.error), [props.onWrite])

    document.querySelectorAll(".editing").forEach((el) => el.classList.remove("editing"))
    if (state?.statement === "UPDATE") {
        state.td.classList.add("editing")
    } else if (state.statement === "DELETE") {
        state.tr.classList.add("editing")
    }

    for (const { buildDispatch } of editors) { buildDispatch(setState, props.sql) }

    const statementSelect = <Select value={state.statement} style={{ paddingLeft: "15px", paddingRight: "15px" }} className="primary" onChange={async (value) => {
        try {
            editors.find(({ statement }) => statement === value)?.open(props.tableName)
        } catch (err) {
            console.error(err)
        }
    }} options={{
        INSERT: {},
        "CREATE TABLE": {},
        "DROP TABLE": {},
        "ALTER TABLE": {},
        UPDATE: { disabled: true, title: "Click a cell" },
        DELETE: { disabled: true, title: "Click a row number" }
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
export type EditorComponent<T> = (props: { statementSelect: JSXInternal.Element, state: T, setState: (newState: T) => void, commit: (query: string, params: DataTypes[], opts: OnWriteOptions) => void }) => JSXInternal.Element
