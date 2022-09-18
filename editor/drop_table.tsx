import { Commit } from "./components"
import { escapeSQLIdentifier } from "../main"
import { DispatchBuilder, EditorComponent, TitleComponent } from "."

export const statement = "DROP TABLE"
export type State = Readonly<{
    statement: typeof statement,
    tableName: string
}>
export declare const state: State

export let open: (tableName?: string) => Promise<void>
export const buildDispatch: DispatchBuilder<State> = (setState) => open = async (tableName) => {
    if (tableName === undefined) { return }
    setState({ statement, tableName })
}

export const Title: TitleComponent<State> = (props) =>
    <> {escapeSQLIdentifier(props.state.tableName)}</>

export const Editor: EditorComponent<State> = (props) =>
    <pre style={{ paddingTop: "4px" }}><Commit onClick={() => props.commit(`DROP TABLE ${escapeSQLIdentifier(props.state.tableName)}`, [])} /></pre>
