import { Commit } from "./components"
import { DispatchBuilder, EditorComponent } from "."
import { useState } from "preact/hooks"

export const statement = "custom"
export type State = Readonly<{
    statement: typeof statement,
}>
export declare const state: State

export let open: () => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async () => {
    setState({ statement })
}

export const Editor: EditorComponent<State> = (props) => {
    const [query, setQuery] = useState("")
    return <pre>
        <h2>{props.statementSelect}</h2>
        <textarea autocomplete="off" style={{ marginTop: "15px", width: "100%", height: "20vh", resize: "none" }} placeholder={"CREATE TABLE table1(column1 INTEGER)"} value={query} onChange={(ev) => { setQuery(ev.currentTarget.value) }}></textarea>
        <Commit onClick={() => props.commit(query, [], { refreshTableList: true })} />
    </pre>
}
