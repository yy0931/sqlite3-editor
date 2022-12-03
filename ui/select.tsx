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
        <span style={{ userSelect: "none", display: "inline-block", pointerEvents: "none", width: 0, height: 0, overflow: "hidden" }}>
            <select autocomplete="off" value={props.value} style={{ paddingLeft: "15px", paddingRight: "15px", ...props.style, visibility: "hidden" }} className={props.className} ref={ref2} onChange={(ev) => props.onChange(ev.currentTarget.value as T)}>
                <option value={props.value} tabIndex={-1}>{props.options[props.value].text ?? props.value}</option>
            </select>
        </span>
    </>
}
