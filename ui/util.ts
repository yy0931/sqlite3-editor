import create, { StoreApi, UseBoundStore } from "zustand"

export const createStore = <S, A>(name: string, initialState: S, actions: (
    set: UseBoundStore<StoreApi<S>>["setState"],
    get: UseBoundStore<StoreApi<S>>["getState"],
) => A) => fixZustandHMR(name, create<S & A>()((set, get) => ({ ...initialState, ...actions(set, get) })))

const fixZustandHMR = <T>(key: string, store: T): T => {
    // @ts-ignore
    if (import.meta.env.DEV) {
        return (window as any)[`zustand-${key}`] ?? ((window as any)[`zustand-${key}`] = store)
    }
    return store
}

export const BigintMath = {
    max: (...args: bigint[]) => args.reduce((prev, curr) => curr > prev ? curr : prev),
    min: (...args: bigint[]) => args.reduce((prev, curr) => curr < prev ? curr : prev),
}
