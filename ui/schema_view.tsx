import Prism from "prismjs"
import "prismjs/components/prism-sql"
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

    // {index.partial ? " PARTIAL" : ""}<br />
    // {index.origin === "c" ? "Origin: CREATE INDEX" : index.origin === "pk" ? "Origin: PRIMARY KEY" : index.origin === "u" ? "Origin: UNIQUE" : ""}<br />

    return <>
        <h2 className="font-bold">Schema</h2>
        {tableName && <div className="mb-2">
            {tableType === "table" && <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2"
                style={{ borderBottom: "1px solid gray", background: editorStatement === "DROP TABLE" ? "rgba(100, 100, 100)" : "", color: editorStatement === "DROP TABLE" ? "white" : "" }}
                onClick={() => {
                    if (editorStatement === "DROP TABLE") {
                        useEditorStore.getState().cancel().catch(console.error)
                        return
                    }
                    useEditorStore.getState().dropTable(tableName)
                }}>
                <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#trash" /></svg>
                <span className="ml-1">{"Drop Table"}</span>
            </span>}
            {tableType === "view" && <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2"
                style={{ borderBottom: "1px solid gray", background: editorStatement === "DROP VIEW" ? "rgba(100, 100, 100)" : "", color: editorStatement === "DROP VIEW" ? "white" : "" }}
                onClick={() => {
                    if (editorStatement === "DROP VIEW") {
                        useEditorStore.getState().cancel().catch(console.error)
                        return
                    }
                    useEditorStore.getState().dropView(tableName)
                }}>
                <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#trash" /></svg>
                <span className="ml-1">{"Drop View"}</span>
            </span>}
            <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2"
                style={{ borderBottom: "1px solid gray", background: editorStatement === "ALTER TABLE" ? "rgba(100, 100, 100)" : "", color: editorStatement === "ALTER TABLE" ? "white" : "" }}
                onClick={() => {
                    if (editorStatement === "ALTER TABLE") {
                        useEditorStore.getState().cancel().catch(console.error)
                        return
                    }
                    useEditorStore.getState().alterTable(tableName, undefined).catch(console.error)
                }}>
                <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#edit" /></svg>
                <span className="ml-1">{"Alter Table"}</span>
            </span>
        </div>}
        <div className="[padding-left:var(--page-padding)] mb-4"><pre className="[font-size:inherit] overflow-x-auto bg-white p-2" dangerouslySetInnerHTML={{ __html: Prism.highlight(schema ?? "", Prism.languages.sql!, "sql") }}></pre></div>
        <h2 className="font-bold">Indexes</h2>
        {tableName && <div className="mb-2">
            <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer ml-2"
                style={{ borderBottom: "1px solid gray", background: editorStatement === "CREATE INDEX" ? "rgba(100, 100, 100)" : "", color: editorStatement === "CREATE INDEX" ? "white" : "" }}
                onClick={() => {
                    if (editorStatement === "CREATE INDEX") {
                        useEditorStore.getState().cancel().catch(console.error)
                        return
                    }
                    useEditorStore.getState().createIndex(tableName)
                }}>
                <svg className="inline [width:1em] [height:1em]"><use xlinkHref="#add" /></svg>
                <span className="ml-1">{"Create Index"}</span>
            </span>
        </div>}
        <div className="[padding-left:var(--page-padding)]">
            <ul className="list-disc ml-4">
                {indexList.map((index, i) => {
                    return <li>
                        {index.name.startsWith("sqlite_") ? <b><i>{index.name}</i></b> : <b>{index.name}</b>}
                        {!index.name.startsWith("sqlite_") && <span className="align-middle hover:bg-gray-300 active:bg-inherit select-none pl-1 pr-1 [border-radius:1px] inline-block cursor-pointer ml-2"
                            style={{ borderBottom: "1px solid gray" }}
                            title="Drop Index"
                            onClick={() => { useEditorStore.getState().dropIndex(tableName, index.name) }}>
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
        </div>
    </>
}
