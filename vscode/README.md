# SQLite3 Editor
Edit SQLite3 files like you would in Excel.

**IMPORTANT**: This extension requires **Python 3.7 or higher**.

This extension uses the `sqlite3` module in the standard library of Python to query sqlite3 databases. It searches through the PATH for a Python 3 binary, but if it can't find one or the wrong version of Python is selected, you can specify the filepath of a python binary in the config `sqlite3-editor.pythonPath`.

## Screenshot
![](https://raw.githubusercontent.com/yy0931/sqlite3-editor/main/screenshot.png)

## Features
- **Supported statements**: ALTER TABLE, CREATE TABLE, DELETE, DROP TABLE, DROP VIEW, INSERT, UPDATE, CREATE INDEX, DROP INDEX, and custom queries.
- **Click a cell and UPDATE in-place** with an intuitive GUI.
- **Find widget** to filter records with **regex**, **whole word**, and **case-sensitivity** switches.
- **Efficiently** edit large tables by **only querying the visible area**.
- **Auto-reload** when the table is modified by another process.

# Usage
This extension recognizes `.db`, `.sqlite`, and `.sqlite3` files as Sqlite3 databases. To open other files, add the following settings to your [user or workspace settings.json](https://code.visualstudio.com/docs/getstarted/settings):

```json
// Associates *.ext to the SQLite3 editor
"workbench.editorAssociations": {
    "*.ext": "sqlite3-editor.editor"
},
```