import type { JSXInternal } from "preact/src/jsx"
import { useRef, Ref, useLayoutEffect, useEffect, useState } from "preact/hooks"
import SQLite3Client, { DataTypes } from "../sql"
import { blob2hex, escapeSQLIdentifier, type2color } from "../main"

export type EditorDataType = "string" | "number" | "null" | "blob"

export const DataTypeInput = (props: { value: EditorDataType, onChange: (value: EditorDataType) => void }) =>
    <Select value={props.value} onChange={props.onChange} tabIndex={-1} options={{ string: { text: "TEXT" }, number: { text: "NUMERIC" }, null: { text: "NULL" }, blob: { text: "BLOB" } }} />

export const DataEditor = (props: { rows?: number, style?: JSXInternal.CSSProperties, ref?: Ref<HTMLTextAreaElement & HTMLInputElement>, type: EditorDataType, textareaValue: string, setTextareaValue: (value: string) => void, blobValue: Uint8Array | null, setBlobValue: (value: Uint8Array) => void, tabIndex?: number, sql: SQLite3Client }) => {
    const [filename, setFilename] = useState("")
    if (props.type === "blob") {
        return <div>
            <input value={"x'" + blob2hex(props.blobValue ?? new Uint8Array(), 8) + "'"} disabled={true} style={{ marginRight: "10px" }} />
            <input value={filename} placeholder={"tmp.dat"} onInput={(ev) => setFilename(ev.currentTarget.value)} />
            <input type="button" value="Import" onClick={() => props.sql.import(filename).then((data) => props.setBlobValue(data))} disabled={filename === ""} />
            <input type="button" value="Export" onClick={() => props.sql.export(filename, props.blobValue ?? new Uint8Array())} disabled={filename === "" || props.blobValue === null} />
        </div>
    }
    if (props.type === "string") {
        return <textarea
            ref={props.ref}
            rows={props.rows}
            autocomplete="off"
            style={{ color: type2color(props.type), resize: props.type === "string" ? "vertical" : "none", ...props.style }}
            value={props.textareaValue}
            onInput={(ev) => props.setTextareaValue(ev.currentTarget.value)}
            tabIndex={props.tabIndex} />
    }
    return <input
        ref={props.ref}
        autocomplete="off"
        style={{ color: type2color(props.type), display: "block" }}
        value={props.type === "null" ? "NULL" : props.textareaValue}
        onInput={(ev) => props.setTextareaValue(ev.currentTarget.value)}
        disabled={props.type === "null"}
        tabIndex={props.tabIndex} />
}

export const parseTextareaValue = (value: string, blobValue: Uint8Array | null, type: EditorDataType): DataTypes => {
    if (type === "null") {
        return null
    } else if (type === "number") {
        if (/^[+\-]?\d+$/.test(value.trim()) && -(2n ** 64n / 2n) <= BigInt(value) && BigInt(value) <= 2n ** 64n / 2n - 1n) {
            return BigInt(value)  // i64
        } else {
            return +value
        }
    } else if (type === "blob") {
        return blobValue ?? new Uint8Array()
    } else {
        return value
    }
}

export const Select = <T extends string>(props: { options: Record<T, { text?: string, disabled?: boolean, disabledReason?: string }>, value: T, onChange: (value: T) => void, style?: JSXInternal.CSSProperties, tabIndex?: number, className?: string }) => {
    const ref1 = useRef() as Ref<HTMLSelectElement>
    const ref2 = useRef() as Ref<HTMLSelectElement>
    useLayoutEffect(() => {
        // Auto resizing https://stackoverflow.com/questions/20091481/auto-resizing-the-select-element-according-to-selected-options-width
        if (!ref1.current || !ref2.current) { return }
        ref1.current.style.width = ref2.current.offsetWidth + "px"
    })
    return <>
        <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style }} className={props.className} ref={ref1} onChange={(ev) => props.onChange(ev.currentTarget.value as T)} tabIndex={props.tabIndex}>{
            (Object.keys(props.options) as T[]).map((value) => <option value={value} disabled={props.options[value].disabled} title={props.options[value].disabled ? props.options[value].disabledReason : undefined}>{props.options[value].text ?? value}</option>)
        }</select>
        <span style={{ userSelect: "none", display: "inline-block", pointerEvents: "none", width: 0, height: 0, overflow: "hidden" }}>
            <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style, visibility: "hidden" }} className={props.className} ref={ref2} onChange={(ev) => props.onChange(ev.currentTarget.value as T)}>
                <option value={props.value} tabIndex={-1}>{props.options[props.value].text ?? props.value}</option>
            </select>
        </span>
    </>
}

export const Commit = (props: { disabled?: boolean, onClick: () => void, style?: JSXInternal.CSSProperties }) => {
    useEffect(() => {
        const handler = (ev: KeyboardEvent) => {
            if (ev.ctrlKey && ev.code === "KeyS") {
                props.onClick()
                ev.preventDefault()
            }
        }
        window.addEventListener("keydown", handler)
        return () => { window.removeEventListener("keydown", handler) }
    }, [props.onClick])
    return <input disabled={props.disabled} type="button" value="Commit" style={{ display: "block", ...props.style }} className={"primary"} onClick={props.onClick} title="Ctrl+S"></input>
}

export const Checkbox = (props: { style?: JSXInternal.CSSProperties, checked: boolean, onChange: (value: boolean) => void, text: string, tabIndex?: number }) =>
    <label style={{ marginRight: "8px", ...props.style }}><input type="checkbox" checked={props.checked} onChange={(ev) => props.onChange(ev.currentTarget.checked)} tabIndex={props.tabIndex}></input> {props.text}</label>

export type ColumnDef = {
    name: string
    affinity: "TEXT" | "NUMERIC" | "INTEGER" | "REAL" | "BLOB" | "ANY"
    primary: boolean
    autoIncrement: boolean
    unique: boolean
    notNull: boolean
}

export const ColumnDefEditor = (props: { columnNameOnly?: boolean, value: ColumnDef, onChange: (columnDef: ColumnDef) => void }) => {
    return <>
        <input tabIndex={0} placeholder="column-name" style={{ marginRight: "8px" }} value={props.value.name} onInput={(ev) => { props.onChange({ ...props.value, name: ev.currentTarget.value }) }}></input>
        {!props.columnNameOnly && <>
            <Select tabIndex={0} style={{ marginRight: "8px" }} value={props.value.affinity} onChange={(value) => props.onChange({ ...props.value, affinity: value })} options={{ "TEXT": {}, "NUMERIC": {}, "INTEGER": {}, "REAL": {}, "BLOB": {}, "ANY": {} }} />
            <Checkbox tabIndex={-1} checked={props.value.primary} onChange={(checked) => props.onChange({ ...props.value, primary: checked })} text="PRIMARY KEY" />
            <Checkbox tabIndex={-1} checked={props.value.autoIncrement} onChange={(checked) => props.onChange({ ...props.value, autoIncrement: checked })} text="AUTOINCREMENT" />
            <Checkbox tabIndex={-1} checked={props.value.unique} onChange={(checked) => props.onChange({ ...props.value, unique: checked })} text="UNIQUE" />
            <Checkbox tabIndex={-1} checked={props.value.notNull} onChange={(checked) => props.onChange({ ...props.value, notNull: checked })} text="NOT NULL" />
        </>}
    </>
}

export const printColumnDef = (def: ColumnDef) =>
    `${escapeSQLIdentifier(def.name)} ${def.affinity}${def.primary ? " PRIMARY KEY" : ""}${def.autoIncrement ? " AUTOINCREMENT" : ""}${def.unique ? " UNIQUE" : ""}${def.notNull ? " NOT NULL" : ""}`