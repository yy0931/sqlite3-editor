import { test, expect, Page } from '@playwright/test'

test('CREATE TABLE', async ({ page }) => {
    await page.goto('http://localhost:5173/')

    /* Check if the CREATE TABLE editor is visible. */ await expect(page.getByTestId("CREATE TABLE")).toBeVisible()
    /* Input a table name. */await page.getByTestId('CREATE TABLE > table-name').fill('test-table')

    /* Set the name     of the 1st column. */await page.getByTestId('column 1').getByTestId('column-name').fill('text-column')
    /* Set the datatype of the 1st column to TEXT. */await page.getByTestId('column 1').getByTestId('column-datatype').selectOption('TEXT')
    /* Set the name     of the 2nd column. */await page.getByTestId('column 2').getByTestId('column-name').fill('integer-column')
    /* Set the datatype of the 2nd column to INTEGER. */await page.getByTestId('column 2').getByTestId('column-datatype').selectOption('INTEGER')
    /* Set the name     of the 3rd column. */await page.getByTestId('column 3').getByTestId('column-name').fill('real-column')
    /* Set the datatype of the 3rd column to REAL. */await page.getByTestId('column 3').getByTestId('column-datatype').selectOption('REAL')
    /* Set the name     of the 4th column. */await page.getByTestId('column 4').getByTestId('column-name').fill('blob-column')
    /* Set the datatype of the 4th column to BLOB. */await page.getByTestId('column 4').getByTestId('column-datatype').selectOption('BLOB')
    /* Set the name     of the 5th column. */await page.getByTestId('column 5').getByTestId('column-name').fill('any-column')
    /* Set the datatype of the 5th column to ANY. */await page.getByTestId('column 5').getByTestId('column-datatype').selectOption('ANY')

    /* Check if the correct sql query is generated. */await expect(page.getByTitle('CREATE TABLE "test-table" ("text-column" TEXT, "integer-column" INTEGER, "real-column" REAL, "blob-column" BLOB, "any-column" ANY) STRICT')).toBeVisible()
    /* Click the commit button. */await page.getByTestId('commit').click()
    /* Check if the table is created. */await expect(page.getByTestId('table-name')).toBeVisible()

    /* Check if the 1st column is displayed in the table header. */await expect(page.getByText('text-column TEXT')).toBeVisible()
    /* Check if the 2nd column is displayed in the table header. */await expect(page.getByText('integer-column INTEGER')).toBeVisible()
    /* Check if the 3rd column is displayed in the table header. */await expect(page.getByText('real-column REAL')).toBeVisible()
    /* Check if the 4th column is displayed in the table header. */await expect(page.getByText('blob-column BLOB')).toBeVisible()
    /* Check if the 5th column is displayed in the table header. */await expect(page.getByText('any-column ANY')).toBeVisible()
})

test.describe("INSERT", () => {
    test('INSERT > single record', async ({ page }) => {
        await page.goto('http://localhost:5173/')

        /* Check if the INSERT editor is visible. */await expect(page.getByTestId("INSERT")).toBeVisible()
        /* Input a value for the text-column. */await page.getByTestId('insert-column 1').getByTestId('editor-textarea').fill('hello')
        /* Input a value for the integer-column. */await page.getByTestId('insert-column 2').getByTestId('editor-textarea').fill('12345');
        /* Input a value for the real-column. */await page.getByTestId('insert-column 3').getByTestId('editor-textarea').fill('1.234');
        /* Input a value for the any-column. */await page.getByTestId('insert-column 5').getByTestId('editor-textarea').fill('test');
        /* Check if the correct sql query is generated. */await expect(page.getByTitle('INSERT INTO "test-table" ("text-column", "integer-column", "real-column", "any-column") VALUES (?, ?, ?, ?)')).toBeVisible()
        /* Press Ctrl+Enter to commit. */await page.getByTestId('insert-column 5').getByTestId('editor-textarea').press('Control+Enter')

        /* Check if the data on the 1st column is displayed in a cell. */await expect(page.getByTestId('viewer-table').getByText('hello')).toBeVisible()
        /* Check if the data on the 2nd column is displayed in a cell. */await expect(page.getByTestId('viewer-table').getByText('12345')).toBeVisible()
        /* Check if the data on the 3rd column is displayed in a cell. */await expect(page.getByTestId('viewer-table').getByText('1.234')).toBeVisible()
        /* Check if the data on the 4th column is displayed in a cell. */await expect(page.getByTestId('viewer-table').getByText('NULL')).toBeVisible()
        /* Check if the data on the 5th column is displayed in a cell. */await expect(page.getByTestId('viewer-table').getByText('test')).toBeVisible()
    })

    test("INSERT > multiple records", async ({ page }) => {
        await page.goto('http://localhost:5173/')

        /* Check if the INSERT editor is visible. */await expect(page.getByTestId("INSERT")).toBeVisible()

        for (let i = 2; i <= 50; i++) {
            /* Input a value. */await page.getByTestId('insert-column 1').getByTestId('editor-textarea').fill(`row ${i}`)
            /* Check if the correct sql query is generated. */await expect(page.getByTitle('INSERT INTO "test-table" ("text-column") VALUES (?)')).toBeVisible()
            /* Commit. */await page.getByTestId('insert-column 1').getByTestId('editor-textarea').press('Control+Enter')
            /* Check if the table is scrolled down to the bottom. */await expect(page.getByTestId(`row number ${i}`)).toBeVisible()
        }
    })
})

/** Selects a cell. */
const selectCell = async ({ page }: { page: Page }, row: number, column: number) => {
    /* Click a cell. */await page.getByTestId(`cell ${row}, ${column}`).click()
    /* Checks if the cell is selected. */await expect(page.getByTestId(`cell ${row}, ${column}`).getByTestId("inplaceInput")).toBeVisible()
    /* Check if the UPDATE editor is opened. */await expect(page.getByTestId("UPDATE")).toBeVisible()
}

test("Key Bindings > ArrowDown", async ({ page }) => {
    await page.goto('http://localhost:5173/')
    await selectCell({ page }, 0, 0)
    /* Press ArrowDown. */await page.getByTestId('body').press("ArrowDown")
    /* Checks if the cell (1, 0) is selected. */await expect(page.getByTestId(`cell 1, 0`).getByTestId("inplaceInput")).toBeVisible()
})

test.describe("UPDATE", () => {
    test("Edit in-place, press Enter on a cell, and press the cancel button", async ({ page }) => {
        await page.goto('http://localhost:5173/')
        await selectCell({ page }, 0, 0)
        /* Start editing. */await page.getByTestId("inplaceInput").click()
        const randomText = "value-cancel"
        /* Input a new value. */await page.getByTestId("inplaceInput").fill(randomText)
        /* Press the Enter key. */await page.getByTestId("inplaceInput").press("Enter")
        /* Click the cancel button. */await page.getByTestId("dialog > cancel").click()
        /* Check if the value in the in-place input has not changed. */await expect(page.getByTestId("inplaceInput")).toHaveValue(randomText)
        /* Checks if the cell (0, 0) is selected. */await expect(page.getByTestId(`cell 0, 0`).getByTestId("inplaceInput")).toBeVisible()
    })

    test("Edit in-place, press Enter on a cell, and press the discard changes button", async ({ page }) => {
        await page.goto('http://localhost:5173/')
        await selectCell({ page }, 0, 0)
        /* Start editing. */await page.getByTestId("inplaceInput").click()
        const randomText = "value-discard-changes"
        /* Input a new value. */await page.getByTestId("inplaceInput").fill(randomText)
        /* Press the Enter key. */await page.getByTestId("inplaceInput").press("Enter")
        /* Click the discard changes button. */await page.getByTestId("dialog > discard-changes").click()
        /* Checks if the cell (1, 0) is selected. */await expect(page.getByTestId(`cell 1, 0`).getByTestId("inplaceInput")).toBeVisible()
        /* Check if the previous changes are discarded. */await expect(page.getByTestId('cell 1, 0')).not.toContainText(randomText)
    })

    test("Edit in-place, press Enter on a cell, and press the commit changes button", async ({ page }) => {
        await page.goto('http://localhost:5173/')
        await selectCell({ page }, 0, 0)
        /* Start editing. */await page.getByTestId("inplaceInput").click()
        const randomText = "value-commit-changes"
        /* Input a new value. */await page.getByTestId("inplaceInput").fill(randomText)
        /* Press the Enter key. */await page.getByTestId("inplaceInput").press("Enter")
        /* Click the commit button. */await page.getByTestId("dialog > commit").click()
        /* Check if the changes are committed. */await expect(page.getByTestId('cell 0, 0').getByText(randomText)).toBeVisible()
        /* Checks if the cell (1, 0) is selected. */await expect(page.getByTestId(`cell 1, 0`).getByTestId("inplaceInput")).toBeVisible()
    })

    test("Edit in the large textarea, press Enter in the textarea, and press ctrl+Enter", async ({ page }) => {
        await page.goto('http://localhost:5173/')
        await selectCell({ page }, 0, 0)
        /* Start editing. */await page.getByTestId("inplaceInput").click()
        const randomText = "value-commit-multiline"
        /* Input a new value. */await page.getByTestId("editor-textarea").fill(randomText)
        /* Press the Enter key. */await page.getByTestId("editor-textarea").press("Enter")
        /* Press Ctrl+Enter. */await page.getByTestId('editor-textarea').press('Control+Enter')
        /* Check if the changes are committed. */await expect(page.getByTestId('cell 0, 0').getByText(randomText + "\\n")).toBeVisible()
        /* Checks if the cell (0, 0) is not selected. */await expect(page.getByTestId(`cell 0, 0`).getByTestId("inplaceInput")).not.toBeVisible()
    })
})

const setupTable = async ({ page, tableName, columnNames }: { page: Page, tableName: string, columnNames: string[] }) => {
    /* Click the CREATE TABLE button. */await page.getByTestId("create-table-button").click()
    /* Check if the CREATE TABLE editor is visible. */await expect(page.getByTestId("CREATE TABLE")).toBeVisible()
    /* Input a table name. */await page.getByTestId('CREATE TABLE > table-name').fill(tableName)
    for (const [i, name] of columnNames.entries()) {
        /* Set the name     of the 1st column. */await page.getByTestId(`column ${i + 1}`).getByTestId('column-name').fill(name)
        // FIXME: wait 
        /* Commit. */await page.getByTestId('body').press('Control+Enter')
    }
    /* Check if the newly created table is active. */await expect(page.getByTestId("table-name")).toHaveValue(tableName)
}

test("DROP TABLE", async ({ page }) => {
    await page.goto('http://localhost:5173/')
    await setupTable({ page, tableName: "drop-table", columnNames: ["column1"] })
    /* Open the DROP TABLE editor. */await page.getByTestId("drop-table-button").click()
    /* Check if the DROP TABLE editor is visible. */await expect(page.getByTestId("DROP TABLE")).toBeVisible()
    /* Commit. */await page.getByTestId("body").press("Control+Enter")
    /* Check if the table is dropped. */await expect(page.getByTestId("table-name")).not.toHaveValue("drop-table")
})

test.describe("ALTER TABLE", () => {
    test("ALTER TABLE RENAME TO", async ({ page }) => {
        await page.goto('http://localhost:5173/')
        await setupTable({ page, tableName: "alter-table-rename-to", columnNames: ["column1"] })
        /* Open the ALTER TABLE editor. */await page.getByTestId("alter-table-button").click()
        /* Check if the value of the table-name input is equal to the name of the active table. */await expect(page.getByTestId("alter-table-rename-to-new-table-name")).toHaveValue("alter-table-rename-to")
        /* Input the new table name. */await page.getByTestId("alter-table-rename-to-new-table-name").fill("alter-table-rename-to-renamed")
        /* Check if the correct sql query is generated. */await expect(page.getByTitle('ALTER TABLE alter-table-rename-to RENAME TO alter-table-rename-to-renamed')).toBeVisible()
        /* Commit. */await page.getByTestId("body").press("Control+Enter")
        /* Check if the table is renamed. */await expect(page.getByTestId("table-name")).toHaveValue("alter-table-rename-to-renamed")
    })
})
