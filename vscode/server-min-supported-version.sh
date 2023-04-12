#!/bin/sh

# Use this file to test the server with the minimum supported version of Python and SQLite, by adding the following entry in settings.json.
# "sqlite3-editor.pythonPath": "/path/to/server-min-supported-version.sh"

podman run -i --rm -v /tmp:/tmp -v "$PWD:$PWD" -w "$PWD" docker.io/python:3.6.0 python $@
