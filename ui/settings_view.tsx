import Prism from "prismjs"
import "prismjs/components/prism-sql"
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
                        <b>{index.name}</b><br />
                        {/* if the index is created by CREATE INDEX　*/ indexSchema[i] && indexSchema[i]}
                        {!indexSchema[i] && <>
                            {index.unique ? "UNIQUE " : ""}({indexInfo[i]!
                                .sort((a, b) => Number(a.seqno - b.seqno))
                                .map((info) => info.cid === -2n ? "<expression>" : info.cid === -1n ? "rowid" : info.cid === 0n ? info.name : "Parse Error").join(", ")})
                        </>}
                    </li>
                })}
            </ul>
        </div>
    </>
}
