import type { JSXInternal } from "preact/src/jsx"
import { useRef, Ref, useLayoutEffect } from "preact/hooks"
import { DataTypes } from "../sql"
import { escapeSQLIdentifier } from "../main"

export type EditorDataType = "string" | "number" | "null" | "blob"

export const DataTypeInput = (props: { value: EditorDataType, onChange: (value: EditorDataType) => void }) =>
    <Select value={props.value} onChange={props.onChange} tabIndex={-1} options={{ string: { text: "TEXT" }, number: { text: "NUMERIC" }, null: { text: "NULL" }, blob: { text: "BLOB" } }} />

export const parseTextareaValue = (value: string, type: EditorDataType): DataTypes => {
    if (type === "null") {
        return null
    } else if (type === "number") {
        return +value
    } else if (type === "blob") {
        return Uint8Array.from(value.match(/.{1, 2}/g)?.map((byte) => parseInt(byte, 16)) ?? /* TODO: Show an error message*/[])
    } else {
        return value
    }
}

export const Select = <T extends string>(props: { options: Record<T, { text?: string, disabled?: boolean, title?: string }>, value: T, onChange: (value: T) => void, style?: JSXInternal.CSSProperties, tabIndex?: number, className?: string }) => {
    const ref1 = useRef() as Ref<HTMLSelectElement>
    const ref2 = useRef() as Ref<HTMLSelectElement>
    useLayoutEffect(() => {
        // Auto resizing https://stackoverflow.com/questions/20091481/auto-resizing-the-select-element-according-to-selected-options-width
        if (!ref1.current || !ref2.current) { return }
        ref1.current.style.width = ref2.current.offsetWidth + "px"
    })
    if (props.options[props.value] === undefined) { console.log(props.options, props.value) }
    return <>
        <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style }} className={props.className} ref={ref1} onChange={(ev) => props.onChange(ev.currentTarget.value as T)} tabIndex={props.tabIndex}>{
            (Object.keys(props.options) as T[]).map((value) => <option value={value} disabled={props.options[value].disabled} title={props.options[value].title}>{props.options[value].text ?? value}</option>)
        }</select>
        <span style={{ userSelect: "none", display: "inline-block", pointerEvents: "none", width: 0, height: 0, overflow: "hidden" }}>
            <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style, visibility: "hidden" }} className={props.className} ref={ref2} onChange={(ev) => props.onChange(ev.currentTarget.value as T)}>
                <option value={props.value} tabIndex={-1}>{props.options[props.value].text ?? props.value}</option>
            </select>
        </span>
    </>
}

export const Commit = (props: { onClick: () => void }) =>
    <input type="button" value="Commit" style={{ display: "block", marginTop: "15px", fontSize: "125%" }} className={"primary"} onClick={props.onClick}></input>

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

export const ColumnDefEditor = (props: { value: ColumnDef, onChange: (columnDef: ColumnDef) => void }) => {
    return <>
        <input tabIndex={0} placeholder="column-name" style={{ marginRight: "8px" }} value={props.value.name} onInput={(ev) => { props.onChange({ ...props.value, name: ev.currentTarget.value }) }}></input>
        <Select tabIndex={0} style={{ marginRight: "8px" }} value={props.value.affinity} onChange={(value) => props.onChange({ ...props.value, affinity: value })} options={{ "TEXT": {}, "NUMERIC": {}, "INTEGER": {}, "REAL": {}, "BLOB": {}, "ANY": {} }} />
        <Checkbox tabIndex={-1} checked={props.value.primary} onChange={(checked) => props.onChange({ ...props.value, primary: checked })} text="PRIMARY KEY" />
        <Checkbox tabIndex={-1} checked={props.value.autoIncrement} onChange={(checked) => props.onChange({ ...props.value, autoIncrement: checked })} text="AUTOINCREMENT" />
        <Checkbox tabIndex={-1} checked={props.value.unique} onChange={(checked) => props.onChange({ ...props.value, unique: checked })} text="UNIQUE" />
        <Checkbox tabIndex={-1} checked={props.value.notNull} onChange={(checked) => props.onChange({ ...props.value, notNull: checked })} text="NOT NULL" />
    </>
}

export const printColumnDef = (def: ColumnDef) =>
    `${escapeSQLIdentifier(def.name)} ${def.affinity}${def.primary ? " PRIMARY KEY" : ""}${def.autoIncrement ? " AUTOINCREMENT" : ""}${def.unique ? " UNIQUE" : ""}${def.notNull ? " NOT NULL" : ""}`
