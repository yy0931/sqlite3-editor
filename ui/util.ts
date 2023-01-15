import create, { StoreApi, UseBoundStore } from "zustand"

export const createStore = <S, A>(
    initialState: S,
    actions: (set: UseBoundStore<StoreApi<S>>["setState"], get: UseBoundStore<StoreApi<S>>["getState"]) => A,
) =>
    create<S & A>()((set, get) => ({
        ...initialState,
        ...actions(set, get),
    }))
