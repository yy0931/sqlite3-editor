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
