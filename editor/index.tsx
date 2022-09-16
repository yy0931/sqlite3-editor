import { useState } from "preact/hooks"
import * as immer from "immer"

immer.enableMapSet()

import * as insert from "./insert"
import * as createTable from "./create_table"
import * as update from "./update"
import { Select } from "./components"

export type State =
    | insert.State
    | createTable.State
    | update.State

export const Editor = (props: { refreshTable: () => void }) => {
    const [state, setState] = useState<State | null>(null)

    document.querySelectorAll(".editing").forEach((el) => el.classList.remove("editing"))
    if (state?.statement === "UPDATE") {
        state.td.classList.add("editing")
    }

    insert.init(setState)
    createTable.init(setState)
    update.init(setState)

    if (state === null) { return <></> }

    return <>
        <h2>
            <pre>
                <Select value={state.statement} style={{ color: "white", background: "var(--accent-color)", paddingLeft: "15px", paddingRight: "15px" }} onChange={async (value) => {
                    try {
                        const nextStatement = value
                        switch (nextStatement) {
                            case "INSERT": insert.open(); break
                            case "UPDATE": throw new Error()
                            case "CREATE TABLE": createTable.open(); break
                            default: { const _: never = nextStatement }
                        }
                    } catch (err) {
                        console.error(err)
                    }
                }} options={{ INSERT: {}, "CREATE TABLE": {}, UPDATE: { disabled: true, title: "Click a cell to change a cell value" } }} />
                <span id="editorTitle">{(() => {
                    switch (state.statement) {
                        case "INSERT": return <insert.Title state={state} setState={setState} refreshTable={props.refreshTable} />
                        case "CREATE TABLE": return <createTable.Title state={state} setState={setState} refreshTable={props.refreshTable} />
                        case "UPDATE": return <update.Title state={state} setState={setState} refreshTable={props.refreshTable} />
                        default: { const _: never = state; throw new Error() }
                    }
                })()}</span>
            </pre>
        </h2>
        {(() => {
            switch (state.statement) {
                case "INSERT": return <insert.Editor state={state} setState={setState} refreshTable={props.refreshTable} />
                case "CREATE TABLE": return <createTable.Editor state={state} setState={setState} refreshTable={props.refreshTable} />
                case "UPDATE": return <update.Editor state={state} setState={setState} refreshTable={props.refreshTable} />
                default: { const _: never = state; throw new Error() }
            }
        })()}
    </>
}
