import { Commit } from "./components"
import { escapeSQLIdentifier } from "../main"
import { DispatchBuilder, EditorComponent } from "."

export const statement = "DROP TABLE"
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
    <pre>
        <h2>
            {props.statementSelect}{" "}{escapeSQLIdentifier(props.state.tableName)}
        </h2>
        <Commit onClick={() => props.commit(`DROP TABLE ${escapeSQLIdentifier(props.state.tableName)}`, [], { refreshTableList: true })} />
    </pre>
