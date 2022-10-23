import http.server
import sqlite3
import traceback

from umsgpack import packb, unpack

# FIXME: Escape filename
file = "../node/samples/employees_db-full-1.0.6.db"
readonly_connection = sqlite3.connect("file:" + file + "?mode=ro", uri=True)
readwrite_connection = sqlite3.connect(file)


class Handler(http.server.BaseHTTPRequestHandler):
    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Credentials', 'true')
        self.send_header('Access-Control-Allow-Origin', 'http://localhost:5173')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header("Access-Control-Allow-Headers", "X-Requested-With, Content-type")
        self.end_headers()

    def do_POST(self):
        request_body = unpack(self.rfile)
        response_body = None
        try:
            if self.path == "/query":
                if request_body["mode"] == "w+":
                    with readwrite_connection as con:
                        con.execute(request_body["query"], request_body["params"])
                else:
                    cursor = readonly_connection.execute(request_body["query"], request_body["params"])
                    columns = [desc[0] for desc in cursor.description]
                    response_body = [{k: v for k, v in zip(columns, record)} for record in cursor.fetchall()]
            elif self.path == "/import":
                with open(request_body["filepath"], "rb") as f:
                    response_body = bytearray(f.read())
            elif self.path == "/export":
                with open(request_body["filepath"], "wb") as f:
                    f.write(request_body["data"])
        except Exception as err:
            traceback.print_exc()
            self.send_response(400)
            self.send_header('Access-Control-Allow-Credentials', 'true')
            self.send_header('Access-Control-Allow-Origin', 'http://localhost:5173')
            self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
            self.send_header("Access-Control-Allow-Headers", "X-Requested-With, Content-type")
            self.end_headers()
            self.wfile.write(str(err).encode())
        else:
            self.send_response(200)
            self.send_header('Access-Control-Allow-Credentials', 'true')
            self.send_header('Access-Control-Allow-Origin', 'http://localhost:5173')
            self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
            self.send_header("Access-Control-Allow-Headers", "X-Requested-With, Content-type")
            self.end_headers()
            self.wfile.write(packb(response_body))


print("http://localhost:8080")
http.server.HTTPServer(("localhost", 8080), Handler).serve_forever()
