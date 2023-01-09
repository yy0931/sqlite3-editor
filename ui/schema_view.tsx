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

    return <>
        <h2 class="font-bold">Schema</h2>
        <div class="pl-[var(--page-padding)] mb-4"><pre class="[font-size:inherit] overflow-x-auto bg-white p-2" dangerouslySetInnerHTML={{ __html: Prism.highlight(schema ?? "", Prism.languages.sql!, "sql") }}></pre></div>
        <h2 class="font-bold">Indexes</h2>
        <div class="pl-[var(--page-padding)]">
            <ul class="list-disc ml-4">
                {indexList.map((index, i) => {
                    return <li>
                        {index.name.startsWith("sqlite_") ? <b><i>{index.name}</i></b> : <b>{index.name}</b>}
                        {!index.name.startsWith("sqlite_") && <SVGCheckbox icon="#trash" class="ml-2 pl-1 pr-0" checked={false} title="Drop Index" onClick={() => { useEditorStore.getState().dropIndex(tableName, index.name) }}></SVGCheckbox>}
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
