import Prism from "prismjs"
import "prismjs/components/prism-sql"
import { useTableStore } from "./table"

export const SettingsView = () => {
    const schema = useTableStore((state) => state.tableSchema)

    return <>
        <h2 className="font-bold">Schema</h2>
        <div className="[padding-left:var(--page-padding)] mb-4"><pre className="[font-size:inherit] overflow-x-auto bg-white p-2" dangerouslySetInnerHTML={{ __html: Prism.highlight(schema ?? "", Prism.languages.sql!, "sql") }}></pre></div>
        <h2 className="font-bold">Indexes</h2>
        <div className="[padding-left:var(--page-padding)]">TODO</div>
    </>
}
