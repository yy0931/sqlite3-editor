import { DataTypes } from "../main"
import type { JSXInternal } from "preact/src/jsx"

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

export const Select = <T extends string>(props: { options: Record<T, { text?: string, disabled?: boolean, title?: string }>, value: T, onChange: (value: T) => void, style?: JSXInternal.CSSProperties, tabIndex?: number }) =>
    <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style }} onChange={(ev) => props.onChange(ev.currentTarget.value as T)} tabIndex={props.tabIndex}>{
        (Object.keys(props.options) as T[]).map((value) => <option value={value} disabled={props.options[value].disabled} title={props.options[value].title}>{props.options[value].text ?? value}</option>)
    }</select>

export const Commit = (props: { onClick: () => void }) =>
    <input type="button" value="Commit" style={{ display: "block", marginTop: "15px", fontSize: "125%", color: "white", background: "var(--accent-color)" }} onClick={props.onClick}></input>

export const Checkbox = (props: { style?: JSXInternal.CSSProperties, checked: boolean, onChange: (value: boolean) => void, text: string }) =>
    <label style={{ marginRight: "8px", ...props.style }}><input type="checkbox" checked={props.checked} onChange={(ev) => props.onChange(ev.currentTarget.checked)}></input> {props.text}</label>
