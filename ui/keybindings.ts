import * as editor from "./editor"
import { useTableStore } from "./table"
import { BigintMath } from "./util"

const isInSingleClickState = () => document.querySelector(".single-click") !== null

/** Moves the selection up if delta < 0n, down if delta > 0n. */
const moveSelectionRow = async (delta: bigint) => {
    const state = editor.useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    const { paging, setPaging } = useTableStore.getState()
    let newRelativeRow = BigintMath.min(BigInt(state.row) + delta, paging.numRecords - paging.visibleAreaTop - 1n)

    // Scroll down
    if (newRelativeRow >= paging.visibleAreaSize) {
        const scrollDistance = newRelativeRow - paging.visibleAreaSize + 1n
        await setPaging({ visibleAreaTop: paging.visibleAreaTop + scrollDistance }, true)
        newRelativeRow -= scrollDistance
    }

    // Scroll up
    if (newRelativeRow < 0n) {
        const scrollDistance = -newRelativeRow
        await setPaging({ visibleAreaTop: paging.visibleAreaTop - scrollDistance }, true)
        newRelativeRow += scrollDistance
    }

    state.update(state.tableName, state.column, Number(newRelativeRow))
}

/** Moves the selection left if delta < 0, right if delta > 0. */
const moveSelectionColumn = (delta: number) => {
    const state = editor.useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    const { tableInfo } = useTableStore.getState()
    const columnIndex = Math.max(0, Math.min(tableInfo.length - 1, tableInfo.findIndex(({ name }) => name === state.column) + delta))
    state.update(state.tableName, tableInfo[columnIndex]!.name, state.row)
}

// https://gist.github.com/yy0931/14f854aae53dfd4c090fab389b448844
type Code = "Abort" | "Again" | "AltLeft" | "AltRight" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "ArrowUp" | "AudioVolumeDown" | "AudioVolumeMute" | "AudioVolumeUp" | "Backquote" | "Backslash" | "Backspace" | "BracketLeft" | "BracketRight" | "BrightnessUp" | "BrowserBack" | "BrowserFavorites" | "BrowserForward" | "BrowserHome" | "BrowserRefresh" | "BrowserSearch" | "BrowserStop" | "CapsLock" | "Comma" | "ContextMenu" | "ControlLeft" | "ControlRight" | "Convert" | "Copy" | "Cut" | "Delete" | "Digit0" | "Digit1" | "Digit2" | "Digit3" | "Digit4" | "Digit5" | "Digit6" | "Digit7" | "Digit8" | "Digit9" | "Eject" | "End" | "Enter" | "Equal" | "Escape" | "F1" | "F10" | "F11" | "F12" | "F13" | "F14" | "F15" | "F16" | "F17" | "F18" | "F19" | "F2" | "F20" | "F21" | "F22" | "F23" | "F24" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "Find" | "Fn" | "Help" | "Home" | "Insert" | "IntlBackslash" | "IntlRo" | "IntlYen" | "KanaMode" | "KeyA" | "KeyB" | "KeyC" | "KeyD" | "KeyE" | "KeyF" | "KeyG" | "KeyH" | "KeyI" | "KeyJ" | "KeyK" | "KeyL" | "KeyM" | "KeyN" | "KeyO" | "KeyP" | "KeyQ" | "KeyR" | "KeyS" | "KeyT" | "KeyU" | "KeyV" | "KeyW" | "KeyX" | "KeyY" | "KeyZ" | "Lang1" | "Lang2" | "Lang3" | "Lang4" | "Lang5" | "LaunchApp1" | "LaunchApp2" | "LaunchMail" | "MediaPlayPause" | "MediaSelect" | "MediaStop" | "MediaTrackNext" | "MediaTrackPrevious" | "MetaLeft" | "MetaRight" | "Minus" | "NonConvert" | "NumLock" | "Numpad0" | "Numpad1" | "Numpad2" | "Numpad3" | "Numpad4" | "Numpad5" | "Numpad6" | "Numpad7" | "Numpad8" | "Numpad9" | "NumpadAdd" | "NumpadComma" | "NumpadDecimal" | "NumpadDivide" | "NumpadEnter" | "NumpadEqual" | "NumpadMultiply" | "NumpadParenLeft" | "NumpadParenRight" | "NumpadSubtract" | "OSLeft" | "OSRight" | "Open" | "PageDown" | "PageUp" | "Paste" | "Pause" | "Period" | "Power" | "PrintScreen" | "Props" | "Quote" | "ScrollLock" | "Select" | "Semicolon" | "ShiftLeft" | "ShiftRight" | "Slash" | "Sleep" | "Space" | "Tab" | "Undo" | "Unidentified" | "VolumeDown" | "VolumeMute" | "VolumeUp" | "WakeUp"

export const onKeydown = async (ev: KeyboardEvent) => {
    if (useTableStore.getState().isConfirmDialogVisible) {
        return
    }
    try {
        const state = editor.useEditorStore.getState()
        const singleClick = isInSingleClickState()
        const key = (pattern: `${"Ctrl + " | "" | "(Ctrl +) "}${"Shift + " | "" | "(Shift +) "}${"Alt + " | "" | "(Alt +) "}${Code}`): boolean => {
            const m = /^(Ctrl \+ |\(Ctrl \+\) |)(Shift \+ |\(Shift \+\) |)(Alt \+ |\(Alt \+\) |)(.*?)$/.exec(pattern)
            if (m === null) { throw new Error() }
            const [_, c, s, a, code] = m
            if (c === "Ctrl + " && !ev.ctrlKey) { return false }
            if (c === "" && ev.ctrlKey) { return false }
            if (s === "Shift + " && !ev.shiftKey) { return false }
            if (s === "" && ev.shiftKey) { return false }
            if (a === "Alt + " && !ev.altKey) { return false }
            if (a === "" && ev.altKey) { return false }
            return ev.code === code
        }
        const inputFocus = ev.target instanceof HTMLElement && !ev.target.matches("table textarea") && ev.target.matches("label, button, input, textarea, select, option")
        const findWidgetFocus = ev.target instanceof HTMLInputElement && ev.target.id === "findWidget"

        let preventDefault = true
        if (findWidgetFocus && key("Escape")) {
            ev.target.blur()
            await useTableStore.getState().setFindWidgetVisibility(false)
        } else if (inputFocus && key("Escape")) {
            ev.target.blur()
        } else if (key("Ctrl + KeyF")) {
            if (useTableStore.getState().isFindWidgetVisible) {
                const findWidget = document.querySelector<HTMLInputElement>("#findWidget")
                if (findWidget) {
                    findWidget.focus()
                    findWidget.select()
                }
            } else {
                await useTableStore.getState().setFindWidgetVisibility(true)
            }
        } else if (findWidgetFocus && key("Alt + KeyC")) {
            const mainState = useTableStore.getState()
            await mainState.setFindWidgetState({ caseSensitive: !mainState.findWidget.caseSensitive })
        } else if (findWidgetFocus && key("Alt + KeyW")) {
            const mainState = useTableStore.getState()
            await mainState.setFindWidgetState({ wholeWord: !mainState.findWidget.wholeWord })
        } else if (findWidgetFocus && key("Alt + KeyR")) {
            const mainState = useTableStore.getState()
            await mainState.setFindWidgetState({ regex: !mainState.findWidget.regex })
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("Escape")) {
            await state.clearInputs()
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("Escape")) {
            state.update(state.tableName, state.column, state.row)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("ArrowUp")) {
            await moveSelectionRow(-1n)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("ArrowDown")) {
            await moveSelectionRow(1n)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("ArrowLeft")) {
            moveSelectionColumn(-1)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("ArrowRight")) {
            moveSelectionColumn(1)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && (key("Ctrl + ArrowUp") || key("Ctrl + Home"))) {
            await moveSelectionRow(-useTableStore.getState().paging.numRecords)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && (key("Ctrl + ArrowDown") || key("Ctrl + End"))) {
            await moveSelectionRow(useTableStore.getState().paging.numRecords)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && (key("Ctrl + ArrowLeft") || key("Home"))) {
            moveSelectionColumn(-useTableStore.getState().tableInfo.length)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && (key("Ctrl + ArrowRight") || key("End"))) {
            moveSelectionColumn(useTableStore.getState().tableInfo.length)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("PageUp")) {
            await moveSelectionRow(-useTableStore.getState().paging.visibleAreaSize)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("PageDown")) {
            await moveSelectionRow(useTableStore.getState().paging.visibleAreaSize)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("(Shift +) Enter")) {
            document.querySelector(".single-click")!.classList.remove("single-click")
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("Enter")) {
            await editor.useEditorStore.getState().commitUpdate(true)
            await moveSelectionRow(1n)
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("Shift + Enter")) {
            await editor.useEditorStore.getState().commitUpdate(true)
            await moveSelectionRow(-1n)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("Tab")) {
            moveSelectionColumn(1)
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("Tab")) {
            await editor.useEditorStore.getState().commitUpdate(true)
            moveSelectionColumn(1)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("Shift + Tab")) {
            moveSelectionColumn(-1)
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("Shift + Tab")) {
            await editor.useEditorStore.getState().commitUpdate(true)
            moveSelectionColumn(-1)
        } else if (!inputFocus && state.statement === "DELETE" && key("Escape")) {
            await state.clearInputs()
        } else {
            preventDefault = false
        }
        if (preventDefault) {
            ev.preventDefault()
        }
    } catch (err) {
        console.error(err)
    }
}
