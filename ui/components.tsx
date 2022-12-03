import { useLayoutEffect, useRef, Ref } from "preact/hooks"
import type { JSXInternal } from "preact/src/jsx"

/** Typed version of `<select>` */
export const Select = <T extends string>(props: { options: Record<T, { text?: string, disabled?: boolean, disabledReason?: string }>, value: T, onChange: (value: T) => void, style?: JSXInternal.CSSProperties, tabIndex?: number, className?: string }) => {
    const ref1 = useRef() as Ref<HTMLSelectElement>
    const ref2 = useRef() as Ref<HTMLSelectElement>
    useLayoutEffect(() => {
        // Auto resize the width of `<select>` base on the current value: https://stackoverflow.com/questions/20091481/auto-resizing-the-select-element-according-to-selected-options-width
        if (!ref1.current || !ref2.current) { return }
        ref1.current.style.width = ref2.current.offsetWidth + "px"
    })  // skip the second argument
    return <>
        <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style }} className={props.className} ref={ref1} onChange={(ev) => props.onChange(ev.currentTarget.value as T)} tabIndex={props.tabIndex}>{
            (Object.keys(props.options) as T[]).map((value) => <option value={value} disabled={props.options[value].disabled} title={props.options[value].disabled ? props.options[value].disabledReason : undefined}>{props.options[value].text ?? value}</option>)
        }</select>

        {/* Hidden replication for auto resizing */}
        <span className="select-none inline-block pointer-events-none w-0 h-0 overflow-hidden">
            <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style, visibility: "hidden" }} className={props.className} ref={ref2} onChange={(ev) => props.onChange(ev.currentTarget.value as T)}>
                <option value={props.value} tabIndex={-1}>{props.options[props.value].text ?? props.value}</option>
            </select>
        </span>
    </>
}

export const Button = (props: { className?: string, disabled?: boolean, value?: string, style?: JSXInternal.CSSProperties, title?: string, onClick?: () => void }) => {
    return <input type="button" disabled={props.disabled} value={props.value} style={props.style} className={"border-0 outline-0 pl-2 pr-2 cursor-pointer " + (props.className ?? "")} onClick={props.onClick} title={props.title}></input>
}
