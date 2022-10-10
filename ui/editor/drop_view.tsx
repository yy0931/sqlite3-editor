import { Commit } from "./components"
import { escapeSQLIdentifier } from "../main"
import { DispatchBuilder, EditorComponent } from "."

export const statement = "DROP VIEW"
export type State = Readonly<{
    statement: typeof statement,
    tableName: string
}>
export declare const state: State

export let open: (tableName?: string) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState, sql) => open = async (tableName) => {
    if (tableName === undefined) { return }
    setState({ statement, tableName })
}

export const Editor: EditorComponent<State> = (props) =>
    <>
        <h2>
            {props.statementSelect}{" "}{escapeSQLIdentifier(props.state.tableName)}
        </h2>
        <div>
            <Commit style={{ marginBottom: "10px" }} onClick={() => props.commit(`DROP VIEW ${escapeSQLIdentifier(props.state.tableName)}`, [], { refreshTableList: true })} />
        </div>
    </>