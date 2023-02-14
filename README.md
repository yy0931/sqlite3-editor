**Feel free to report any issues or feature requests.**

----

# SQLite3 Editor

[![GitHub issues](https://img.shields.io/github/issues/yy0931/sqlite3-editor)](https://github.com/yy0931/sqlite3-editor/issues)
[![GitHub closed issues](https://img.shields.io/github/issues-closed/yy0931/sqlite3-editor)](https://github.com/yy0931/sqlite3-editor/issues)
[![GitHub](https://img.shields.io/github/license/yy0931/sqlite3-editor)](https://github.com/yy0931/sqlite3-editor)
[![Github Stars](https://img.shields.io/github/stars/yy0931/sqlite3-editor?style=social)](https://github.com/yy0931/sqlite3-editor)
[![Tweet](https://img.shields.io/twitter/url/http/shields.io.svg?style=social)](https://twitter.com/intent/tweet?text=%23vscode%20extension%20for%20%23sqlite%20&url=https://github.com/yy0931/sqlite3-editor&hashtags=developers)

This extension lets you edit SQLite 3 files without having to write SQL queries.

**IMPORTANT**: This extension requires **Python >=3.6** compiled with SQLite >=3.37.0 (fully supported) or >=3.8.8 (partially supported). The extension will tell you if the requirements are not met.

# Features
## Spreadsheet GUI for Browsing and Editing Data
This extension offers several features for editing SQLite databases intuitively:
- You can **click a cell and UPDATE data in place** with a spreadsheet GUI, even through a view or a custom query (*1).
- This extension uses **scrolling** instead of pagination for browsing records, by **only querying the visible area**, avoiding performance degradation.
- The data is **automatically reloaded** when the table is modified by another process.

These features are not present in other VSCode extensions for SQLite, such as [alexcvzz/SQLite](https://github.com/AlexCovizzi/vscode-sqlite/) and [SQLite Viewer](https://github.com/qwtel/sqlite-viewer-vscode).

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/demo.gif)

> The database in the screenshot is the sample database in MySQL ported to SQLite, downloaded from https://github.com/fracpete/employees-db-sqlite.

> *1: To edit data through a view or a custom query, you need to build [a helper program](https://github.com/yy0931/sqlite3_column_origin) to access the SQLite C/C++ interface. Everything else works fine without the helper program.

## Advanced Query Editor
Although this extension was initially created to fill the need for an extension that could intuitively edit SQLite databases using a GUI, it also comes with an advanced query editor that supports **auto-completion, syntax highlighting, hover information, and signature help**.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/query-editor.gif)

To access the query editor, click the "Custom Query" button.

The line comment at the first line of the query editor indicates which database the editor is connected to, and should not be deleted, or the query editor will be disconnected from the database.
The comment serves the purpose of enabling the user to save the content of the query editor as a file and reuse it later.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/query-editor-file.gif)

There are two ways to execute statements in the query editor:
- Use **Shift+Enter** or click the **"Execute"** button above the statement to run a single statement. It will display the result only if it is a SELECT statement. To display the result of PRAGMA statements, use the pragma functions (e.g. use `SELECT * FROM pragma_table_list();` instead of `PRAGMA table_list;`).
- Use **Ctrl(Cmd)+Shift+E** to run all statements in the query editor. It is useful for running multiple statements.

The extension checks the database's mtime every second to automatically reload the table, which may result in a slight delay between the execution of a statement and the updates to the table.

## Switching Tables
You can switch between tables by clicking a table name in either the editor or the explorer.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/switch-table-2.gif)

When the `sqlite3-editor.nativeTableSelector` configuration option is set to true, the quick pick widget will be displayed instead of a drop-down menu. It is faster to render many tables but is less intuitive to use.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/switch-table-1.gif)

## Filtering and Sorting Data
This extension also includes a **find widget** that enables you to filter records using **regex**, **whole word**, and **case-sensitivity** switches, making data searching faster than using the query editor to write WHERE clauses.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/find_widget.png)

To sort records by a specific column, simply click on the column name. You can also sort the table from the context menu on the column name.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/sorting.png)

## Multi-Selection
UPDATE and DELETE statements support multi-selection. To select multiple cells, you can either drag over the cells, click a cell with the alt key pressed, or press the Shift + Arrow keys while a cell is selected.

> Dragging on row numbers is not currently supported, and you need to use the alt + click.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/multi-select.gif)

## Table Schema Viewer
By clicking the "Schema" button located next to the table name, you can view the schema of the table, as well as its indexes and triggers.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/table-schema.png)

## Full 64-bit integer support
This extension is capable of processing 64-bit integers without any loss of information, which is not straightforward in JavaScript, the language used to implement the UI, as its number type only supports 53-bit integers. To prevent any rounding of large integers, such as occurs in [SQLite Viewer](https://github.com/qwtel/sqlite-viewer-vscode/issues/24), we store all integer values as [bigints](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt).

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/i64.png)

## CSV/JSON/SQL Import/Export
By clicking the "Other Tools..." button, you can access various features, including CSV, JSON, and SQL imports and exports. These features require you to install the [sqlite-utils](https://github.com/simonw/sqlite-utils) package.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/other-tools.png)

## File Associations
This extension recognizes `.db`, `.sqlite`, `.sqlite3`, and `.duckdb` files as database files. To open other files, right-click the file in the explorer and select `Open with…` then `SQLite3 Editor`.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/open_with.gif)

## Links to Tables or Queries
You can create links to specific tables or queries using [query strings](https://en.wikipedia.org/wiki/Query_string).
Note that VSCode's automatic linking, which is enabled by the "editor.links" configuration, may drop query parameters in relative links.

```markdown
# Link to a table
file:///path/to/database.db?tableName=table1

# Link to a SQL query
file:///path/to/database.db?query=SELECT%20c1%20FROM%20table1
```

## Creating a Database
To create a database, simply create a file with a supported file extension, such as .db, .sqlite, .sqlite3, etc. The extension will automatically initialize the file as a database if it is empty and has one of the supported extensions.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/new-database.gif)

## GUI Editors for CREATE TABLE, ALTER TABLE, CREATE INDEX, CREATE TABLE, DROP TABLE, and INSERT
To access the GUI editors for these statements, click the name of the current statement then choose them in the context menu.

![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/editors.png)

## Configuration
### sqlite3-editor.pythonPath
This extension automatically selects a Python with the latest version of the database in the system's PATH. However, if an unexpected version of Python is selected or if you want to use a Python that is not in the PATH, you can use this setting.

The binary specified with this configuration must be a CPython (the standard one that can be obtained from python.org) or a PyPy. The extension does not accept other types of Python or a path to a virtual environment.

### sqlite3-editor.helperProgramPath
Some advanced features require building [a helper program](https://github.com/yy0931/sqlite3_column_origin) to access the SQLite C/C++ interface. This setting is used to set the location of the built program for the extension.

### sqlite3-editor.maxHistoryEntries
By default, this extension saves the last 500 SQL queries in [ExtensionContext.globalState](https://code.visualstudio.com/api/references/vscode-api#ExtensionContext.globalState) and you can view it using `SQLite3 Editor: Show History` and clear it using `SQLite3 Editor: Clear History` from the command palette. This setting specifies the number of queries to save. If set to 0, history will not be saved.

### sqlite3-editor.connectionSetupQueries.driver.{sqlite,duckdb}
This configuration specifies the SQL statements that are executed immediately after connecting to a database.

The keys are case-sensitive regular expressions used to match the file URI (e.g. `file:///path/to/database.db`), and the values are corresponding SQL statements to be executed. If multiple item's patterns match the same URI, only the first item will be used.

This configuration is similar to the "sqlite.setupDatabase" configuration in [alexcvzz/sqlite](https://marketplace.visualstudio.com/items?itemName=alexcvzz.vscode-sqlite) but differs in that it uses regular expressions for path comparison.

As this extension uses Python to connect to databases, commands such as `.load` that are available in the SQLite's CLI will not work.

For example, to execute [`PRAGMA foreign_keys = ON;`](https://www.sqlite.org/pragma.html#pragma_foreign_keys) and [`PRAGMA busy_timeout = 1000;`](https://www.sqlite.org/pragma.html#pragma_busy_timeout) on all SQLite 3 connections, use the following configuration:
```json
"sqlite3-editor.connectionSetupQueries.driver.sqlite3": {
  ".*": "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 1000;"
}
```

### sqlite3-editor.connectionSetupQueries.debug
This setting allows you to view which setup queries were used.
If enabled, the extension displays the URI matched against the regex patterns in connectionSetupQueries and the result of the match for each pattern when an editor is opened.

## Error Reporting
This extension displays a "Send error report" button on error notifications. When clicked, it sends the error message with sensitive information removed to the extension's author using [Microsoft/vscode-extension-telemetry](https://github.com/Microsoft/vscode-extension-telemetry), or opens [a GitHub issue](https://github.com/yy0931/sqlite3-editor), depending on the user's telemetry settings. If vscode-extension-telemetry is used, the system information (i.e. the "common properties" of [Microsoft/vscode-extension-telemetry](https://github.com/Microsoft/vscode-extension-telemetry)) will also be included.

Keep in mind that the bugs may not be fixed if the reports do not include enough information or the developer is busy, but your help in reporting errors is greatly appreciated.

## Telemetry Data
This extension collects anonymous telemetry data using [Microsoft/vscode-extension-telemetry](https://github.com/Microsoft/vscode-extension-telemetry). The following is the list of data collected, and no sensitive or personal information will be collected or transmitted:

- The version numbers of Python and the database engine.
- The type of the editor tab. (file, git-diff, or other)
- The startup time of the editor tab.
- The button pressed in some yes/no dialogs.
- The "common properties" of [Microsoft/vscode-extension-telemetry](https://github.com/Microsoft/vscode-extension-telemetry).

The purpose of this telemetry is to:

- Determine if the software is being used and whether continued development and releases are necessary.
- Improve the software based on the frequently used environments, especially the versions of databases.

The data collected is encrypted and will be deleted after 90 days, which is the default setting in Microsoft Azure Monitor.

The data collected will only be used to improve the extension and will not be shared with any third parties.

VSCode provides the "telemetry.enableTelemetry" configuration option to control telemetry, and this extension, along with any other extensions using [Microsoft/vscode-extension-telemetry](https://github.com/Microsoft/vscode-extension-telemetry), follows this setting.

Please note that the telemetry data collected by this extension may change in future releases.
