import Prism from "prismjs"
import "prismjs/components/prism-sql"
import { useEditorStore } from "./editor"
import { useTableStore } from "./table"

export const SettingsView = () => {
    const schema = useTableStore((state) => state.tableSchema)
    const indexList = useTableStore((state) => state.indexList)
    const indexInfo = useTableStore((state) => state.indexInfo)
    const indexSchema = useTableStore((state) => state.indexSchema)

    // {index.partial ? " PARTIAL" : ""}<br />
    // {index.origin === "c" ? "Origin: CREATE INDEX" : index.origin === "pk" ? "Origin: PRIMARY KEY" : index.origin === "u" ? "Origin: UNIQUE" : ""}<br />

    return <>
        <h2 className="font-bold">Schema</h2>
        <div className="[padding-left:var(--page-padding)] mb-4"><pre className="[font-size:inherit] overflow-x-auto bg-white p-2" dangerouslySetInnerHTML={{ __html: Prism.highlight(schema ?? "", Prism.languages.sql!, "sql") }}></pre></div>
        <h2 className="font-bold">Indexes</h2>
        <div className="[padding-left:var(--page-padding)]">
            <ul className="list-disc ml-4">
                {indexList.map((index, i) => {
                    return <li>
                        {index.name.startsWith("sqlite_") ? <b><i>{index.name}</i></b> : <b>{index.name}</b>}
                        {!index.name.startsWith("sqlite_") && <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-1 pr-1 [border-radius:1px] inline-block cursor-pointer ml-2"
                            title="Drop Index"
                            onClick={() => {
                                const state = useEditorStore.getState()
                                state.dropIndex(state.tableName, index.name)
                            }}>
                            <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#trash" /></svg>
                        </span>}
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
            <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer [margin-left:-0.5rem]"
                onClick={() => {
                    const state = useEditorStore.getState()
                    if (state.tableName === undefined) { throw new Error() }
                    state.createIndex(state.tableName)
                }}>
                <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#add" /></svg>
                <span className="ml-1">{"Create Index"}</span>
            </span>
        </div>
    </>
}
