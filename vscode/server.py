import argparse
import os
import re
import sqlite3
import sys
import traceback
import urllib.parse

from umsgpack import pack, unpack


def find_widget_regexp(text: str, pattern: str, whole_word: int, case_sensitive: int):
    try:
        return 0 if re.search(pattern, "\\b(?:{pattern})\\b" if whole_word else text, 0 if case_sensitive else re.RegexFlag.I) is None else 1
    except re.error:  # Invalid regular expressions
        return 0


class Server:
    def __init__(self, database_filepath, request_body_filepath, response_body_filepath, cwd, tmp_database_path):
        self.readonly_connection = sqlite3.connect("file:" + urllib.parse.quote(database_filepath) + "?mode=ro", uri=True)
        self.readwrite_connection = sqlite3.connect(database_filepath)

        self.readonly_connection.create_function("find_widget_regexp", 4, find_widget_regexp, deterministic=True)
        self.readwrite_connection.create_function("find_widget_regexp", 4, find_widget_regexp, deterministic=True)

        self.readonly_connection.execute("ATTACH DATABASE ? AS editor_tmp_database", ["file:" + urllib.parse.quote(tmp_database_path)])
        self.readwrite_connection.execute("ATTACH DATABASE ? AS editor_tmp_database", ["file:" + urllib.parse.quote(tmp_database_path)])

        self.request_body_filepath = request_body_filepath
        self.response_body_filepath = response_body_filepath
        self.cwd = cwd

    def handle(self, path):
        path = path.strip()
        try:
            with open(self.request_body_filepath, "rb") as f:
                request_body = unpack(f)

            response_body = None

            if path == "/query":
                # { query: string, params: (number | bigint | string | Uint8Array | Buffer)[], mode: "w+" | "r" }
                try:
                    if request_body["mode"] == "w+":
                        with self.readwrite_connection as con:
                            con.execute(request_body["query"], request_body["params"])
                    else:
                        cursor = self.readonly_connection.execute(request_body["query"], request_body["params"])
                        if cursor.description is not None:  # is None when inserting, updating, etc.
                            columns = [desc[0] for desc in cursor.description]
                            response_body = [{k: v for k, v in zip(columns, record)} for record in cursor.fetchall()]
                except Exception as err:
                    raise Exception(f"{err}\nQuery: {request_body['query']}\nParams: {request_body['params']}")
            elif path == "/import":
                # { filepath: string }
                with open(os.path.join(self.cwd, request_body["filepath"]), "rb") as f:
                    response_body = bytearray(f.read())
            elif path == "/export":
                # { filepath: string, data: Uint8Array | Buffer }
                with open(os.path.join(self.cwd, request_body["filepath"]), "wb") as f:
                    f.write(request_body["data"])
            else:
                raise Exception("Invalid path: " + path)
        except Exception as err:
            traceback.print_exc(file=sys.stderr)
            with open(self.response_body_filepath, "w") as f:
                f.write(str(err))
            return 400
        else:
            with open(self.response_body_filepath, "wb") as f:
                pack(response_body, f)
            return 200


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--database-filepath", type=str, required=True)
    parser.add_argument("--request-body-filepath", type=str, required=True)
    parser.add_argument("--response-body-filepath", type=str, required=True)
    parser.add_argument("--cwd", type=str, required=True)
    parser.add_argument("--tmp-database-path", type=str, required=True)
    args = parser.parse_args()
    server = Server(args.database_filepath, args.request_body_filepath, args.response_body_filepath, args.cwd, args.tmp_database_path)
    while True:
        print(server.handle(input()), flush=True)
