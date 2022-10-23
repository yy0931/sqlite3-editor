import sqlite3 from "better-sqlite3"

export default class BetterSqlite3Client {
    readonly #readonlyConnection
    readonly #readWriteConnection

    constructor(databasePath: string) {
        this.#readonlyConnection = sqlite3(databasePath, { readonly: true })
        this.#readWriteConnection = sqlite3(databasePath)
    }

    query(query: string, params: (null | number | bigint | string | Buffer | Uint8Array)[], mode: "r" | "w+") {
        try {
            const statement = (mode === "w+" ? this.#readWriteConnection : this.#readonlyConnection).prepare(query)
            if (statement.reader) {
                return statement.all(...params)
            } else {
                statement.run(...params)
                return undefined
            }
        } catch (err) {
            throw new Error(`${(err as Error).message}\nQuery: ${query}\nParams: ${JSON.stringify(params)}`)
        }
    }

    close() {
        this.#readonlyConnection.close()
        this.#readWriteConnection.close()
    }
}
