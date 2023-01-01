import * as editor from "./editor"
import { useMainStore } from "./main"
import { useTableStore } from "./table"

const isInSingleClickState = () => document.querySelector(".single-click") !== null

const toSingleClick = () => {
    const state = editor.useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    if (!isInSingleClickState()) {
        state.update(state.tableName, state.column, state.row)
    }
}

const moveSelectionUp = async () => {
    const state = editor.useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    const { paging, setPaging } = useMainStore.getState()
    const rowNumber = Number(paging.visibleAreaTop) + state.row
    if (rowNumber >= 1) {
        if (state.row === 0) {
            await setPaging({ visibleAreaTop: paging.visibleAreaTop - 1n }, true)
            state.update(state.tableName, state.column, 0)
        } else {
            state.update(state.tableName, state.column, state.row - 1)
        }
    } else {
        toSingleClick()
    }
}

const moveSelectionDown = async () => {
    const state = editor.useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    const { paging, setPaging } = useMainStore.getState()
    const rowNumber = Number(paging.visibleAreaTop) + state.row
    if (rowNumber <= Number(paging.numRecords) - 2) {
        if (state.row === Number(paging.visibleAreaSize) - 1) {
            await setPaging({ visibleAreaTop: paging.visibleAreaTop + 1n }, true)
            state.update(state.tableName, state.column, Number(paging.visibleAreaSize) - 1)
        } else {
            state.update(state.tableName, state.column, state.row + 1)
        }
    } else {
        toSingleClick()
    }
}

const moveSelectionRight = () => {
    const state = editor.useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    const { tableInfo } = useTableStore.getState()
    const columnIndex = tableInfo.findIndex(({ name }) => name === state.column)

    if (columnIndex <= tableInfo.length - 2) {
        state.update(state.tableName, tableInfo[columnIndex + 1]!.name, state.row)
    } else {
        toSingleClick()
    }
}

const moveSelectionLeft = () => {
    const state = editor.useEditorStore.getState()
    if (state.statement !== "UPDATE") { return }
    const { tableInfo } = useTableStore.getState()
    const columnIndex = tableInfo.findIndex(({ name }) => name === state.column)

    if (columnIndex >= 1) {
        state.update(state.tableName, tableInfo[columnIndex - 1]!.name, state.row)
    } else {
        toSingleClick()
    }
}

export const onKeydown = async (ev: KeyboardEvent) => {
    try {
        const state = editor.useEditorStore.getState()
        const singleClick = isInSingleClickState()
        const key = (pattern: `${"c" | "!c" | ""}${"s" | "!s" | ""}${"a" | "!a" | ""}+${string}`): boolean => {
            const m = /^(!?c)?(!?s)?(!?a)?\+(.*)$/.exec(pattern)
            if (m === null) { throw new Error() }
            const [_, c, s, a, code] = m
            if (c === "c" && !ev.ctrlKey) { return false }
            if (c === "!c" && ev.ctrlKey) { return false }
            if (s === "s" && !ev.shiftKey) { return false }
            if (s === "!s" && ev.shiftKey) { return false }
            if (a === "a" && !ev.altKey) { return false }
            if (a === "!a" && ev.altKey) { return false }
            return ev.code === code
        }
        const inputFocus = !(ev.target instanceof HTMLElement && (ev.target.matches("table textarea") || !ev.target.matches("label, button, input, textarea, select, option")))

        if (inputFocus && key("+Escape")) {
            const { activeElement } = document
            if (activeElement instanceof HTMLElement) {
                activeElement.blur()
            }
        } else if (key("c!s!a+KeyF")) {
            ev.preventDefault()
            const isFindWidgetVisible = useMainStore.getState().isFindWidgetVisibleWhenValueIsEmpty || useMainStore.getState().findWidget.value !== ""
            if (isFindWidgetVisible) {
                const findWidget = document.querySelector<HTMLInputElement>("#findWidget")
                if (findWidget) {
                    findWidget.focus()
                    findWidget.select()
                }
            } else {
                useMainStore.setState({ isFindWidgetVisibleWhenValueIsEmpty: true })
            }
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("+Escape")) {
            ev.preventDefault()
            await state.clearInputs()
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("+Escape")) {
            ev.preventDefault()
            state.update(state.tableName, state.column, state.row)
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("!c!s!a+ArrowUp")) {
            ev.preventDefault()
            await moveSelectionUp()
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("!c!s!a+ArrowDown")) {
            ev.preventDefault()
            await moveSelectionDown()
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("!c!s!a+ArrowLeft")) {
            ev.preventDefault()
            moveSelectionLeft()
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("!c!s!a+ArrowRight")) {
            ev.preventDefault()
            moveSelectionRight()
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("!c!a+Enter")) {
            ev.preventDefault()
            document.querySelector(".single-click")!.classList.remove("single-click")
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("!c!s!a+Enter")) {
            ev.preventDefault()
            await editor.useEditorStore.getState().commitUpdate(true)
            await moveSelectionDown()
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("!cs!a+Enter")) {
            ev.preventDefault()
            await editor.useEditorStore.getState().commitUpdate(true)
            await moveSelectionUp()
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("!c!s!a+Tab")) {
            ev.preventDefault()
            moveSelectionRight()
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("!c!s!a+Tab")) {
            ev.preventDefault()
            await editor.useEditorStore.getState().commitUpdate(true)
            moveSelectionRight()
        } else if (!inputFocus && state.statement === "UPDATE" && singleClick && key("!cs!a+Tab")) {
            ev.preventDefault()
            moveSelectionLeft()
        } else if (!inputFocus && state.statement === "UPDATE" && !singleClick && key("!cs!a+Tab")) {
            ev.preventDefault()
            await editor.useEditorStore.getState().commitUpdate(true)
            moveSelectionLeft()
        } else if (!inputFocus && state.statement === "DELETE" && key("+Escape")) {
            ev.preventDefault()
            await state.clearInputs()
        } else if (!inputFocus && key("!c!s!a+ArrowDown")) {
            ev.preventDefault()
            const mainState = useMainStore.getState()
            await mainState.setPaging({ visibleAreaTop: mainState.paging.visibleAreaTop + 1n })
        } else if (!inputFocus && key("!c!s!a+ArrowUp")) {
            ev.preventDefault()
            const mainState = useMainStore.getState()
            await mainState.setPaging({ visibleAreaTop: mainState.paging.visibleAreaTop - 1n })
        } else if (!inputFocus && key("!c!s!a+PageDown")) {
            ev.preventDefault()
            const mainState = useMainStore.getState()
            await mainState.setPaging({ visibleAreaTop: mainState.paging.visibleAreaTop + mainState.paging.visibleAreaSize })
        } else if (!inputFocus && key("!c!s!a+PageUp")) {
            ev.preventDefault()
            const mainState = useMainStore.getState()
            await mainState.setPaging({ visibleAreaTop: mainState.paging.visibleAreaTop - mainState.paging.visibleAreaSize })
        } else if (!inputFocus && key("c!s!a+Home")) {
            ev.preventDefault()
            const mainState = useMainStore.getState()
            await mainState.setPaging({ visibleAreaTop: 0n })
        } else if (!inputFocus && key("c!s!a+End")) {
            ev.preventDefault()
            const mainState = useMainStore.getState()
            await mainState.setPaging({ visibleAreaTop: mainState.paging.numRecords - mainState.paging.visibleAreaSize })
        }
    } catch (err) {
        console.error(err)
    }
}
