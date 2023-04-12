import create, { StoreApi, UseBoundStore } from "zustand"

export const createStore = <S, A>(name: string, initialState: S, actions: (
    set: UseBoundStore<StoreApi<S>>["setState"],
    get: UseBoundStore<StoreApi<S>>["getState"],
) => A) =>
    hmr(name, create<S & A>()((set, get) => ({
        ...initialState,
        ...actions(set, get)
    })))

const hmr = <T>(key: string, store: T): T => {
    // @ts-ignore
    if (import.meta.env.DEV) {
        return (window as any)[key] ?? ((window as any)[key] = store)
    }
    return store
}

export const BigintMath = {
    max: (...args: bigint[]) => args.reduce((prev, curr) => curr > prev ? curr : prev),
    min: (...args: bigint[]) => args.reduce((prev, curr) => curr < prev ? curr : prev),
}

/**
 * Attempts to find the first element that matches the given CSS selector using `document.querySelector`. If the element is not found, it sets up a MutationObserver to observe the body element and its descendant nodes for mutations. If a matching element is added as a result of a mutation, the MutationObserver is disconnected and the element is returned. The MutationObserver waits for at most one second for a matching element to be added.
 * @param selector - The CSS selector to use.
 * @returns The first element that matches the given selector, or `null` if no such element is found after one second of observing for mutations.
 */
export const querySelectorWithRetry = <T extends Element = Element>(selector: string) => new Promise<T | null>((resolve) => {
    // Attempt to find the element using document.querySelector
    const result = document.querySelector<T>(selector)
    if (result) {
        // If the element was found, return it
        resolve(result)
        return
    }

    // Create a MutationObserver to listen for mutations to the body element or its descendant nodes
    const observer = new MutationObserver((mutations) => {
        // For each mutation
        for (const mutation of mutations) {
            for (const child of mutation.addedNodes) {
                if (!(child instanceof Element)) { continue }
                const result = child.querySelector<T>(selector)
                if (result) {
                    // Disconnect the observer and return the element
                    observer.disconnect()
                    clearTimeout(timeout)
                    resolve(result)
                    return
                }
            }
        }
    })

    // If the element was not found, start observing for mutations
    observer.observe(document.body, { childList: true, subtree: true })

    // Set a timeout to stop observing after one second
    const timeout = setTimeout(() => {
        observer.disconnect()
        resolve(null)
    }, 1000)
})

