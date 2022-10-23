import argparse
import os
import sqlite3
import sys
import traceback
import urllib.parse

from umsgpack import pack, unpack

parser = argparse.ArgumentParser()
parser.add_argument("--database-filepath", type=str, required=True)
parser.add_argument("--request-body-filepath", type=str, required=True)
parser.add_argument("--response-body-filepath", type=str, required=True)
parser.add_argument("--cwd", type=str, required=True)
args = parser.parse_args()

readonly_connection = sqlite3.connect("file:" + urllib.parse.quote(args.database_filepath) + "?mode=ro", uri=True)
readwrite_connection = sqlite3.connect(args.database_filepath)

while True:
    path = input()

    try:
        with open(args.request_body_filepath, "rb") as f:
            request_body = unpack(f)

        response_body = None

        if path == "/query":
            # { query: string, params: (number | bigint | string | Uint8Array | Buffer)[], mode: "w+" | "r" }
            try:
                if request_body["mode"] == "w+":
                    with readwrite_connection as con:
                        con.execute(request_body["query"], request_body["params"])
                else:
                    cursor = readonly_connection.execute(request_body["query"], request_body["params"])
                    columns = [desc[0] for desc in cursor.description]
                    response_body = [{k: v for k, v in zip(columns, record)} for record in cursor.fetchall()]
            except Exception as err:
                raise Exception(f"{err}\nQuery: {request_body['query']}\nParams: {request_body['params']}")
        elif path == "/import":
            # { filepath: string }
            with open(os.path.join(args.cwd, request_body["filepath"]), "rb") as f:
                response_body = bytearray(f.read())
        elif path == "/export":
            # { filepath: string, data: Uint8Array | Buffer }
            with open(os.path.join(args.cwd, request_body["filepath"]), "wb") as f:
                f.write(request_body["data"])
        else:
            print("Invalid path: " + path, file=sys.stderr)
            continue
    except Exception as err:
        traceback.print_exc(file=sys.stderr)
        with open(args.response_body_filepath, "w") as f:
            f.write(str(err))
        print(400, flush=True)
    else:
        with open(args.response_body_filepath, "wb") as f:
            pack(response_body, f)
        print(200, flush=True)
