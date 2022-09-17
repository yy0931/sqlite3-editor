import { useState, useCallback } from "preact/hooks"
import * as immer from "immer"
import type { JSXInternal } from "preact/src/jsx"

immer.enableMapSet()

import * as insert from "./insert"
import * as createTable from "./create_table"
import * as dropTable from "./drop_table"
import * as update from "./update"
import { Select } from "./components"
import { DataTypes, sql } from "../main"

const editors = [insert, createTable, dropTable, update]

export type State = (typeof editors[number])["state"]

export const Editor = (props: { refreshTable: () => void }) => {
    const [state, setState] = useState<State | null>(null)
    const commit = useCallback((query: string, params: DataTypes[]) => sql(query, params, "w+").then(() => props.refreshTable()).catch(console.error), [props.refreshTable])

    document.querySelectorAll(".editing").forEach((el) => el.classList.remove("editing"))
    if (state?.statement === "UPDATE") {
        state.td.classList.add("editing")
    }

    for (const { buildDispatch } of editors) { buildDispatch(setState) }

    if (state === null) { return <></> }

    return <>
        <h2>
            <pre>
                <Select value={state.statement} style={{ color: "white", background: "var(--accent-color)", paddingLeft: "15px", paddingRight: "15px" }} onChange={async (value) => {
                    try {
                        editors.find(({ statement }) => statement === value)?.open()
                    } catch (err) {
                        console.error(err)
                    }
                }} options={{
                    INSERT: {},
                    "CREATE TABLE": {},
                    "DROP TABLE": {},
                    UPDATE: { disabled: true, title: "Click a cell to change a cell value" },
                }} />
                <span id="editorTitle">{(() => {
                    const editor = editors.find(({ statement }) => statement === state.statement)
                    if (!editor) { throw new Error() }
                    return <editor.Title
                        // @ts-ignore
                        state={state}
                        setState={setState} />
                })()}</span>
            </pre>
        </h2>
        {(() => {
            const editor = editors.find(({ statement }) => statement === state.statement)
            if (!editor) { throw new Error() }
            return <editor.Editor
                // @ts-ignore
                state={state}
                setState={setState}
                commit={commit} />
        })()}
    </>
}

export type DispatchBuilder<T> = (setState: (newState: T) => void) => void
export type TitleComponent<T> = (props: { state: T, setState: (newState: T) => void }) => JSXInternal.Element
export type EditorComponent<T> = (props: { state: T, setState: (newState: T) => void, commit: (query: string, params: DataTypes[]) => void }) => JSXInternal.Element
