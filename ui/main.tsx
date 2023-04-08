import { enableMapSet } from "immer"
enableMapSet()

import { render } from "preact"
import { Editor, useEditorStore } from "./editor"
import { useEffect, useRef } from "preact/hooks"
import * as remote from "./remote"
import { Button, Highlight, persistentUseState, Select, SVGCheckbox, SVGOnlyCheckbox } from "./components"
import { Table, useTableStore } from "./table"
import "./scrollbar"
import { SettingsView } from "./schema_view"
import { onKeydown } from "./keybindings"
import { useEventListener, useInterval } from "usehooks-ts"
// @ts-ignore
import codiconTxt from "./codicon.txt?raw"

export type VSCodeAPI = {
    postMessage(data: unknown): void
    getState(): unknown
    setState(value: unknown): void
}

declare global {
    interface Window {
        acquireVsCodeApi?: () => VSCodeAPI
    }
}

/** Looping animation to indicate loading state, visible only when body.querying. */
const LoadingIndicator = () => {
    const ref = useRef<HTMLDivElement>(null)
    let x = 0
    const width = 200
    const t = Date.now()
    useEffect(() => {
        if (ref.current === null) { return }
        let canceled = false
        const loop = () => {
            if (canceled) { return }
            if (document.body.classList.contains("querying")) {
                ref.current!.style.left = `${x}px`
                x = (Date.now() - t) % (window.innerWidth + width) - width
                ref.current!.style.opacity = "1"
            } else {
                ref.current!.style.opacity = "0"
            }
            requestAnimationFrame(loop)
        }
        loop()
        return () => { canceled = true }
    }, [])
    return <div class="progressbar inline-block select-none pointer-events-none absolute top-0 z-[100] h-[5px] bg-[var(--button-primary-background)] opacity-0" ref={ref} style={{ width: width + "px", transition: "opacity 0.5s cubic-bezier(1.000, 0.060, 0.955, -0.120)" }}></div>
}

/** The root element. */
const App = () => {
    const requireReloading = useTableStore((s) => s.requireReloading)
    const isConfirmationDialogVisible = useTableStore((s) => s.isConfirmDialogVisible)
    const errorMessage = useTableStore((s) => s.errorMessage)
    const tableList = useTableStore((s) => s.tableList)
    const setViewerQuery = useTableStore((s) => s.setViewerQuery)
    const setPaging = useTableStore((s) => s.setPaging)
    const isFindWidgetVisible = useTableStore((s) => s.isFindWidgetVisible)
    const autoReload = useTableStore((s) => s.autoReload)
    const useCustomViewerQuery = useTableStore((s) => s.useCustomViewerQuery)
    const customViewerQuery = useTableStore((s) => s.customViewerQuery)
    const tableName = useTableStore((s) => s.tableName)
    const tableType = useTableStore((s) => s.tableList.find(({ name }) => name === tableName)?.type)
    const editorStatement = useEditorStore((s) => s.statement)
    const isTableRendered = useTableStore((s) => s.invalidQuery === null)
    const [isSettingsViewOpen, setIsSettingsViewOpen] = persistentUseState("isSettingsViewOpen", false)
    const confirmDialogRef = useRef<HTMLDialogElement>(null)
    const tableRef = useRef<HTMLDivElement>(null)

    // Show or close the confirmation dialog
    useEffect(() => {
        if (isConfirmationDialogVisible) {
            confirmDialogRef.current?.showModal()
            document.querySelector<HTMLButtonElement>(".confirm-dialog-commit")?.focus()
        } else {
            confirmDialogRef.current?.close()
        }
    }, [isConfirmationDialogVisible])
    useEventListener("close", () => {
        const { isConfirmDialogVisible } = useTableStore.getState()
        if (isConfirmDialogVisible) {
            isConfirmDialogVisible("cancel")
        }
    }, confirmDialogRef)

    // Reload all tables if the database file is updated
    useEventListener("message", ({ data }: remote.Message) => {
        if (data.type === "sqlite3-editor-server" && data.requestId === undefined) {
            if (useTableStore.getState().autoReload) {
                requireReloading()
            }
        }
    })
    useInterval(() => {
        if (useTableStore.getState().reloadRequired) {
            useTableStore.getState().reloadAllTables()
                .catch(console.error)
        }
    }, 1000)

    // Register keyboard shortcuts
    useEventListener("keydown", onKeydown)

    // Commit changes when the margin on the right side of table is clicked.
    useEventListener("click", (ev) => {
        if (!tableRef.current || ev.target !== document.body) { return }
        const tableRect = tableRef.current.getBoundingClientRect()
        if (!(tableRect.top <= ev.clientY && ev.clientY < tableRect.bottom)) { return }
        (async () => {
            if (!await useEditorStore.getState().beforeUnmount()) { return }
            await useEditorStore.getState().discardChanges()
        })().catch(console.error)
    })

    return <>
        {/* @vscode/codicons/dist/codicon.svg, https://github.com/microsoft/vscode-codicons, Attribution 4.0 International */}
        <svg xmlns="http://www.w3.org/2000/svg" xmlnsXlink="http://www.w3.org/1999/xlink" dangerouslySetInnerHTML={{ __html: codiconTxt }}></svg>

        <LoadingIndicator />

        {/* Header `SELECT * FROM ...` */}
        <h2 class="pt-[var(--page-padding)]">
            <div class="mb-2">
                {/* SELECT * FROM ... */}
                {!useCustomViewerQuery && <>
                    <Highlight>SELECT </Highlight>
                    *
                    <Highlight> FROM </Highlight>
                    {tableName === undefined ? <>No tables</> : <Select value={tableName} onChange={(value) => { setViewerQuery({ tableName: value }).catch(console.error) }} options={Object.fromEntries(tableList.map(({ name: tableName, type }) => [tableName, { group: type }] as const).sort((a, b) => a[0].localeCompare(b[0])))} class="primary" data-testid="table-name" />}
                </>}

                {/* Custom Query */}
                {useCustomViewerQuery && <>
                    <input placeholder="SELECT * FROM table-name" class="w-96" value={customViewerQuery} onBlur={(ev) => { setViewerQuery({ customViewerQuery: ev.currentTarget.value }).catch(console.error) }}></input>
                </>}

                {/* Buttons placed right after the table name */}
                <span class="ml-1">
                    {/* Schema */}
                    {!useCustomViewerQuery && <SVGOnlyCheckbox icon={isSettingsViewOpen ? "#close" : "#settings-gear"} title="Show Table Schema and Indexes" checked={isSettingsViewOpen} onClick={() => setIsSettingsViewOpen(!isSettingsViewOpen)} data-testid="schema-button"></SVGOnlyCheckbox>}

                    {/* Drop Table */}
                    {!useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#trash" title="Drop Table" checked={editorStatement === "DROP TABLE"} onClick={(checked) => {
                        if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                        useEditorStore.getState().dropTable(tableName)
                    }} data-testid="drop-table-button"></SVGOnlyCheckbox>}

                    {/* Drop View */}
                    {!useCustomViewerQuery && tableName && tableType === "view" && <SVGOnlyCheckbox icon="#trash" title="Drop View" checked={editorStatement === "DROP VIEW"} onClick={(checked) => {
                        if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                        useEditorStore.getState().dropView(tableName)
                    }} data-testid="drop-view-button"></SVGOnlyCheckbox>}

                    {/* Alter Table */}
                    {!useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#edit" title="Alter Table" checked={editorStatement === "ALTER TABLE"} onClick={(checked) => {
                        if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                        useEditorStore.getState().alterTable(tableName, undefined).catch(console.error)
                    }} data-testid="alter-table-button"></SVGOnlyCheckbox>}

                    {/* Create Table button */}
                    <SVGOnlyCheckbox icon="#empty-window" title="Create Table" checked={editorStatement === "CREATE TABLE"} onClick={(checked) => {
                        if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                        useEditorStore.getState().createTable(tableName)
                    }} data-testid="create-table-button"></SVGOnlyCheckbox>

                    {/* Create Index */}
                    {!useCustomViewerQuery && tableName && tableType === "table" && <SVGOnlyCheckbox icon="#symbol-interface" title="Create Index" checked={editorStatement === "CREATE INDEX"} onClick={(checked) => {
                        if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                        useEditorStore.getState().createIndex(tableName)
                    }} data-testid="create-index-button"></SVGOnlyCheckbox>}

                    {/* Custom Query button */}
                    <SVGOnlyCheckbox icon="#terminal" title="Custom Query" checked={editorStatement === "Custom Query"} onClick={(checked) => {
                        if (!checked) { useEditorStore.getState().cancel().catch(console.error); return }
                        useEditorStore.getState().custom(tableName)
                    }} data-testid="custom-query-button"></SVGOnlyCheckbox>

                    {/* Find */}
                    {isTableRendered && !isSettingsViewOpen && <SVGOnlyCheckbox icon="#search" title="Find (Ctrl+f)" checked={isFindWidgetVisible} onClick={(checked) => {
                        useTableStore.getState().setFindWidgetVisibility(checked).catch(console.error)
                    }} data-testid="find-button"></SVGOnlyCheckbox>}
                </span>

                {/* The checkbox to toggle the custom query mode */}
                <label class="ml-2 select-none cursor-pointer"><input type="checkbox" checked={useCustomViewerQuery} onChange={() => { setViewerQuery({ useCustomViewerQuery: !useCustomViewerQuery }).catch(console.error) }}></input> Custom</label>

                {/* The checkbox to toggle auto reloading */}
                <label class="select-none cursor-pointer ml-2" title="Reload the table when the database is updated by another process."><input type="checkbox" checked={autoReload} onChange={() => { useTableStore.setState({ autoReload: !autoReload }) }}></input> Auto reload</label>
            </div>
        </h2>

        {/* Schema and Index */}
        {isSettingsViewOpen && <div>
            <SettingsView />
            <hr class="mt-2 border-b-2 border-b-gray-400" />
        </div>}

        {/* Table */}
        {!isSettingsViewOpen && <>
            <div ref={tableRef} class="relative w-max max-w-full pl-[var(--page-padding)] pr-[var(--page-padding)]">
                <Table />
            </div>

            {/* The horizontal handle to resize the height of the table */}
            <div class="h-2 cursor-ns-resize select-none" onMouseDown={(ev) => {
                ev.preventDefault()
                document.body.classList.add("ns-resize")
                let prev = ev.pageY
                let visibleAreaSize = useTableStore.getState().paging.visibleAreaSize
                const onMouseMove = (ev: MouseEvent) => {
                    const trHeight = 18  // TODO: measure the height of a tr
                    let pageSizeDelta = 0n
                    while (ev.pageY - prev > trHeight) {
                        pageSizeDelta += 1n
                        prev += trHeight
                    }
                    while (ev.pageY - prev < -trHeight) {
                        pageSizeDelta -= 1n
                        prev -= trHeight
                    }
                    if (pageSizeDelta === 0n) { return }
                    visibleAreaSize += pageSizeDelta
                    setPaging({ visibleAreaSize }).catch(console.error)
                }
                window.addEventListener("mousemove", onMouseMove)
                window.addEventListener("mouseup", () => {
                    window.removeEventListener("mousemove", onMouseMove)
                    document.body.classList.remove("ns-resize")
                }, { once: true })
            }}>
                <hr class="mt-2 border-b-2 border-b-gray-400" />
            </div>
        </>}

        {/* Error Message */}
        {errorMessage && <p class="text-white bg-[rgb(14,72,117)] [padding:10px]">
            <pre class="whitespace-pre-wrap [font-size:inherit] overflow-auto h-28">{errorMessage}</pre>
            <Button class="primary mt-[10px]" onClick={() => useTableStore.setState({ errorMessage: "" })}>Close</Button>
        </p>}

        {/* Editor */}
        <Editor />

        {/* Confirmation Dialog */}
        <dialog class="py-4 px-8 bg-[#f0f0f0] shadow-lg mx-auto mt-[10vh]" ref={confirmDialogRef}>
            <h2 class="pb-2 pl-0 text-center [font-size:120%]">Commit changes to the database?</h2>
            <div class="float-right">
                <Button onClick={() => { if (isConfirmationDialogVisible) { isConfirmationDialogVisible("commit") } }} class="confirm-dialog-commit mr-1" data-testid="dialog > commit">Commit</Button>
                <Button onClick={() => { if (isConfirmationDialogVisible) { isConfirmationDialogVisible("discard changes") } }} class="bg-[var(--dropdown-background)] hover:[background-color:#8e8e8e] mr-1" data-testid="dialog > discard-changes">Discard changes</Button>
                <Button onClick={() => { if (isConfirmationDialogVisible) { isConfirmationDialogVisible("cancel") } }} class="bg-[var(--dropdown-background)] hover:[background-color:#8e8e8e]" data-testid="dialog > cancel">Cancel</Button>
            </div>
        </dialog>
    </>
}

(async () => {
    await remote.downloadState()
    const tableList = await remote.getTableList()
    const tableName = (() => {
        const restored = remote.getState<string>("tableName")
        return restored && tableList.some(({ name }) => name === restored) ?
            restored :
            tableList[0]?.name
    })()
    useTableStore.setState({ tableList })
    await useTableStore.getState().setViewerQuery({ tableName })
    {
        const restored = remote.getState<number>("visibleAreaSize")
        if (restored !== undefined) {
            await useTableStore.getState().setPaging({ visibleAreaSize: BigInt(restored) })
        }
    }
    await useEditorStore.getState().switchTable(tableName)
    render(<App />, document.body)
})().catch((err) => {
    console.error(err)
    document.write(err)
    document.write(useTableStore.getState().errorMessage)
})
