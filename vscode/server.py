import argparse
import re
import sqlite3
import sys
import urllib.parse
from typing import Any

from umsgpack import pack, unpack


def find_widget_regexp(text: str, pattern: str, whole_word: int, case_sensitive: int):
    try:
        return 0 if re.search(pattern, "\\b(?:{pattern})\\b" if whole_word else text, 0 if case_sensitive else re.RegexFlag.I) is None else 1
    except re.error:  # Invalid regular expressions
        return 0

class Server:
    def __init__(self, database_filepath: str):
        self.readonly_connection = sqlite3.connect("file:" + urllib.parse.quote(database_filepath) + "?mode=ro", uri=True)
        self.readwrite_connection = sqlite3.connect(database_filepath)

        if sys.version_info >= (3, 8, 3):  # `deterministic` is added in Python 3.8.3  https://docs.python.org/3/library/sqlite3.html#sqlite3.Connection.create_function
            self.readonly_connection.create_function("find_widget_regexp", 4, find_widget_regexp, deterministic=True)
            self.readwrite_connection.create_function("find_widget_regexp", 4, find_widget_regexp, deterministic=True)
        else:
            self.readonly_connection.create_function("find_widget_regexp", 4, find_widget_regexp)
            self.readwrite_connection.create_function("find_widget_regexp", 4, find_widget_regexp)

    def handle(self, request_body_filepath: str, response_body_filepath: str):
        def return_(status: int, value: Any):
            if status == 200:
                with open(response_body_filepath, "wb") as f:
                    pack(value, f)
            else:
                with open(response_body_filepath, "w") as f:
                    f.write(value)
            return status

        try:
            # request_body: { query: string, params: (null | number | bigint | string | Uint8Array | Buffer)[], mode: "w+" | "r" }
            with open(request_body_filepath, "rb") as f:
                request_body = unpack(f)

            try:
                if request_body["mode"] == "w+":
                    # read-write
                    with self.readwrite_connection as con:
                        con.execute(request_body["query"], request_body["params"])
                        return return_(200, None)
                else:
                    # read-only
                    with self.readonly_connection as con:
                        cursor = con.execute(request_body["query"], request_body["params"])
                    if cursor.description is not None:
                        columns = [desc[0] for desc in cursor.description]
                        return return_(200, {"columns": columns, "records": [{k: v for k, v in zip(columns, record)} for record in cursor.fetchall()]})
                    else:
                        return return_(200, None)
            except Exception as err:
                return return_(400, f"{err}\nQuery: {request_body['query']}\nParams: {request_body['params']}")
        except Exception as err:
            return return_(400, str(err))

    def close(self):
        self.readonly_connection.close()

        # Create a noop checkpoint to delete WAL files. https://www.sqlite.org/wal.html#avoiding_excessively_large_wal_files
        with self.readwrite_connection as con:
            con.execute("SELECT * FROM sqlite_master LIMIT 1").fetchall()
        self.readwrite_connection.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--database-filepath", type=str, required=True)
    parser.add_argument("--request-body-filepath", type=str, required=True)
    parser.add_argument("--response-body-filepath", type=str, required=True)
    args = parser.parse_args()
    server = Server(args.database_filepath)
    try:
        while True:
            command = input().strip()
            if command == "handle":
                print(server.handle(args.request_body_filepath, args.response_body_filepath), flush=True)
            elif command == "close":
                break
    finally:
        server.close()
