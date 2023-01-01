import Prism from "prismjs"
import "prismjs/components/prism-sql"
import { SVGCheckbox } from "./components"
import { useEditorStore } from "./editor"
import { useMainStore } from "./main"
import { useTableStore } from "./table"

export const SettingsView = () => {
    const schema = useTableStore((state) => state.tableSchema)
    const indexList = useTableStore((state) => state.indexList)
    const indexInfo = useTableStore((state) => state.indexInfo)
    const indexSchema = useTableStore((state) => state.indexSchema)
    const tableName = useEditorStore((state) => state.tableName)
    const tableType = useMainStore((state) => state.tableList.find(({ name }) => name === state.tableName)?.type)
    const editorStatement = useEditorStore((state) => state.statement)

    return <>
        <h2 className="font-bold">Schema</h2>
        {tableName && <div className="mb-2">
            {tableType === "table" && <SVGCheckbox icon="#trash" className="ml-2" checked={editorStatement === "DROP TABLE"} onClick={(checked) => {
                if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                useEditorStore.getState().dropTable(tableName)
            }}>Drop Table</SVGCheckbox>}
            {tableType === "view" && <SVGCheckbox icon="#trash" className="ml-2" checked={editorStatement === "DROP VIEW"} onClick={(checked) => {
                if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                useEditorStore.getState().dropView(tableName)
            }}>Drop View</SVGCheckbox>}
            {tableType === "table" && <SVGCheckbox icon="#edit" className="ml-2" checked={editorStatement === "ALTER TABLE"} onClick={(checked) => {
                if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                useEditorStore.getState().alterTable(tableName, undefined).catch(console.error)
            }}>Alter Table</SVGCheckbox>}
        </div>}
        <div className="[padding-left:var(--page-padding)] mb-4"><pre className="[font-size:inherit] overflow-x-auto bg-white p-2" dangerouslySetInnerHTML={{ __html: Prism.highlight(schema ?? "", Prism.languages.sql!, "sql") }}></pre></div>
        <h2 className="font-bold">Indexes</h2>
        {tableName && <div className="mb-2">
            <SVGCheckbox icon="#add" className="ml-2" checked={editorStatement === "CREATE INDEX"} onClick={(checked) => {
                if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                useEditorStore.getState().createIndex(tableName)
            }}>Create Index</SVGCheckbox>
        </div>}
        <div className="[padding-left:var(--page-padding)]">
            <ul className="list-disc ml-4">
                {indexList.map((index, i) => {
                    return <li>
                        {index.name.startsWith("sqlite_") ? <b><i>{index.name}</i></b> : <b>{index.name}</b>}
                        {!index.name.startsWith("sqlite_") && <SVGCheckbox icon="#trash" className="ml-2 pl-1 pr-0" checked={false} title="Drop Index" onClick={() => { useEditorStore.getState().dropIndex(tableName, index.name) }}></SVGCheckbox>}
                        <br />
                        {/* if the index is created by CREATE INDEX　*/ indexSchema[i] && indexSchema[i]}
                        {!indexSchema[i] && <>
                            {index.unique ? "UNIQUE " : ""}({indexInfo[i]!
                                .sort((a, b) => Number(a.seqno - b.seqno))
                                .map((info) => info.cid === -2n ? "<expression>" : info.cid === -1n ? "rowid" : info.name).join(", ")})
                        </>}
                    </li>
                })}
            </ul>
        </div>
    </>
}
