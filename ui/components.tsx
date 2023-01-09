import { useLayoutEffect, useRef, Ref, useState } from "preact/hooks"
import type { JSXInternal } from "preact/src/jsx"
import type { ReadonlyDeep } from "type-fest"
import * as remote from "./remote"

/** Typed version of `<select>` */
export const Select = <T extends string>(props: { options: Record<T, { text?: string, disabled?: boolean, disabledReason?: string, group?: string }>, value: T, onChange: (value: T) => void, style?: JSXInternal.CSSProperties, tabIndex?: number, className?: string }) => {
    const ref1 = useRef() as Ref<HTMLSelectElement>
    const ref2 = useRef() as Ref<HTMLSelectElement>
    useLayoutEffect(() => {
        // Auto resize the width of `<select>` base on the current value: https://stackoverflow.com/questions/20091481/auto-resizing-the-select-element-according-to-selected-options-width
        if (!ref1.current || !ref2.current) { return }
        ref1.current.style.width = ref2.current.offsetWidth + "px"
    })  // skip the second argument

    const groups = new Map<string | undefined, string[]>()
    for (const [value, { group }] of Object.entries(props.options) as [T, (typeof props.options)[T]][]) {
        if (!groups.has(group)) { groups.set(group, []) }
        groups.get(group)!.push(value)
    }

    return <>
        <select autocomplete="off" value={props.value} style={props.style} className={"[padding-left:15px] [padding-right:15px] " + (props.className ?? "")} ref={ref1} onChange={(ev) => props.onChange(ev.currentTarget.value as T)} tabIndex={props.tabIndex}>{
            ([...groups.entries()] as [string, T[]][]).map(([group, values]) => {
                const options = values.map((value) => <option value={value} disabled={props.options[value].disabled} title={props.options[value].disabled ? props.options[value].disabledReason : undefined}>{props.options[value].text ?? value}</option>)
                if (group === undefined) {
                    return options
                }
                return <optgroup label={group}>{options}</optgroup>
            })
        }</select>

        {/* Hidden replication for auto resizing */}
        <span className="select-none inline-block pointer-events-none w-0 h-0 overflow-hidden">
            <select autocomplete="off" tabIndex={-1} value={props.value} style={{ ...props.style, visibility: "hidden" }} className={"[padding-left:15px] [padding-right:15px] " + (props.className ?? "")} ref={ref2} onChange={(ev) => props.onChange(ev.currentTarget.value as T)}>
                <option value={props.value} tabIndex={-1}>{props.options[props.value].text ?? props.value}</option>
            </select>
        </span>
    </>
}

export const Button = (props: { className?: string, disabled?: boolean, children?: preact.ComponentChildren, style?: JSXInternal.CSSProperties, title?: string, onClick?: () => void }) => {
    return <button disabled={props.disabled} style={props.style} className={"border-0 outline-0 pl-2 pr-2 cursor-pointer [font-family:inherit] [background-color:var(--button-primary-background)] [color:var(--button-primary-foreground)] hover:[background-color:var(--button-primary-hover-background)] disabled:cursor-not-allowed disabled:[color:#737373] disabled:[background-color:#c1bbbb] " + (props.className ?? "")} onClick={props.onClick} title={props.title}>{props.children}</button>
}

/** useRef() but persists the value in the server. */
export const persistentRef = <T extends unknown>(key: string, defaultValue: T) => {
    return useState(() => {
        let value: ReadonlyDeep<T> = remote.getState(key) ?? defaultValue as ReadonlyDeep<T>
        return {
            get current(): ReadonlyDeep<T> { return value },
            set current(newValue: ReadonlyDeep<T>) { remote.setState(key, value = newValue).catch(console.error) },
        }
    })[0]
}

export const persistentUseState = <T extends unknown>(key: string, defaultValue: T) => {
    const [state, setState] = useState<ReadonlyDeep<T>>(remote.getState(key) ?? defaultValue as ReadonlyDeep<T>)
    return [state, (value: T) => {
        remote.setState(key, value).catch(console.error)
        setState(value as ReadonlyDeep<T>)
    }] as const
}

export const SVGOnlyCheckbox = (props: { icon: string, checked?: boolean, onClick: (checked: boolean) => void, className?: string, title: string }) => {
    return <span className={"align-middle hover:bg-gray-300 active:bg-inherit select-none px-1 [border-radius:1px] inline-block cursor-pointer " + (props.className ? props.className : "")}
        title={props.title}
        onClick={() => props.onClick(!props.checked)}>
        <svg className="inline [width:1em] [height:1em]"><use xlinkHref={props.icon} /></svg>
    </span>
}

export const SVGCheckbox = (props: { icon: string, tabIndex?: number, checked?: boolean, onClick: (checked: boolean) => void, className?: string, title?: string, children?: preact.ComponentChildren }) => {
    return <span tabIndex={props.tabIndex} className={"align-middle hover:bg-gray-300 active:bg-inherit select-none pl-2 pr-2 [border-radius:1px] inline-block cursor-pointer " + (props.className ? props.className : "")}
        title={props.title}
        onClick={() => props.onClick(!props.checked)}>
        <svg className="inline [width:1em] [height:1em]"><use xlinkHref={props.icon} /></svg>
        <span className="ml-1">{props.children}</span>
    </span>
}

export const Checkbox = (props: { style?: JSXInternal.CSSProperties, checked: boolean, onChange: (value: boolean) => void, text: string, tabIndex?: number, className?: string }) =>
    <label className={"select-none mr-2 cursor-pointer " + (props.className ?? "")} tabIndex={props.tabIndex ?? 0} style={{ borderBottom: "1px solid gray", color: props.checked ? "rgba(0, 0, 0)" : "rgba(0, 0, 0, 0.4)", ...props.style }} onClick={() => props.onChange(!props.checked)} onKeyDown={(ev) => { if (["Enter", "Space"].includes(ev.code)) { props.onChange(!props.checked) } }}>
        {props.text}
    </label>

export const Highlight = (props: { children: preact.ComponentChildren }) => <span className="[color:var(--button-primary-background)]">{props.children}</span>
