import { Commit } from "./components"
import { escapeSQLIdentifier, getTableName } from "../main"
import { DispatchBuilder, EditorComponent, TitleComponent } from "."

export const statement = "DROP TABLE"
export type State = Readonly<{
    statement: typeof statement,
    tableName: string
}>
export declare const state: State

export let open: () => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState) => open = async () =>
    setState({ statement, tableName: getTableName() })

export const Title: TitleComponent<State> = (props) =>
    <> {escapeSQLIdentifier(props.state.tableName)}</>

export const Editor: EditorComponent<State> = (props) =>
    <pre style={{ paddingTop: "4px" }}><Commit onClick={() => props.commit(`DROP TABLE ${escapeSQLIdentifier(props.state.tableName)}`, [])} /></pre>
