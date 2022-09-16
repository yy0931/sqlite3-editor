import { useState } from "preact/hooks"
import * as immer from "immer"

immer.enableMapSet()

import * as insert from "./insert"
import * as createTable from "./create_table"
import * as update from "./update"
import { DataTypes } from "../main"

export type State = insert.State | createTable.State | update.State

export type EditorDataType = "string" | "number" | "null" | "blob"

export const DataTypeInput = (props: { value: EditorDataType, onChange: (value: EditorDataType) => void }) => {
    return <select autocomplete="off" value={props.value} onChange={(ev) => { props.onChange(ev.currentTarget.value as any) }} tabIndex={-1}>
        <option value="string">TEXT</option>
        <option value="number">NUMERIC</option>
        <option value="null">NULL</option>
        <option value="blob">BLOB</option>
    </select>
}

export const parseTextareaValue = (value: string, type: EditorDataType): DataTypes => {
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
                <select autocomplete="off" value={state.statement} style={{ color: "white", background: "var(--accent-color)", paddingLeft: "15px", paddingRight: "15px" }} onChange={async (ev) => {
                    try {
                        const nextStatement = ev.currentTarget.value as State["statement"]
                        switch (nextStatement) {
                            case "INSERT": insert.open(); break
                            case "UPDATE": throw new Error()
                            case "CREATE TABLE": createTable.open(); break
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
                <span id="editorTitle">{(() => {
                    switch (state.statement) {
                        case "INSERT": return <insert.Title state={state} setState={setState} refreshTable={props.refreshTable} />; break
                        case "CREATE TABLE": return <createTable.Title state={state} setState={setState} refreshTable={props.refreshTable} />; break
                        case "UPDATE": return <update.Title state={state} setState={setState} refreshTable={props.refreshTable} />; break
                        default: { const _: never = state; throw new Error() }
                    }
                })()}</span>
            </pre>
        </h2>
        {(() => {
            switch (state.statement) {
                case "INSERT": return <insert.Editor state={state} setState={setState} refreshTable={props.refreshTable} />; break
                case "CREATE TABLE": return <createTable.Editor state={state} setState={setState} refreshTable={props.refreshTable} />; break
                case "UPDATE": return <update.Editor state={state} setState={setState} refreshTable={props.refreshTable} />; break
                default: { const _: never = state; throw new Error() }
            }
        })()}
    </>
}
