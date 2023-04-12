from pathlib import Path

from server import Server
from umsgpack import packb, unpackb

test_dir = Path(__file__).parent/"_test_files"
test_dir.mkdir(parents=True, exist_ok=True)

req = test_dir/"request.msgpack"
res = test_dir/"response.msgpack"

database = test_dir/"database.msgpack"
database.write_text("")
server = Server(str(database))

def test_readonly_query():
    req.write_bytes(packb({"query": "SELECT 1 + 2", "params": [], "mode": "r"}))
    assert server.handle(str(req), str(res)) == 200
    assert unpackb(res.read_bytes()) == {"columns": ["1 + 2"], "records": [{"1 + 2": 3}]}

def test_params():
    req.write_bytes(packb({"query": "SELECT ? + ? as value", "params": [1, 2], "mode": "r"}))
    assert server.handle(str(req), str(res)) == 200
    assert unpackb(res.read_bytes()) == {"columns": ["value"], "records": [{"value": 3}]}

def test_read_write_query():
    req.write_bytes(packb({"query": "CREATE TABLE 'test-table'(x, y)", "params": [], "mode": "w+"}))
    assert server.handle(str(req), str(res)) == 200
    assert unpackb(res.read_bytes()) == None

    req.write_bytes(packb({"query": "INSERT INTO 'test-table' (x, y) VALUES (?, ?)", "params": [10, 20], "mode": "w+"}))
    assert server.handle(str(req), str(res)) == 200
    assert unpackb(res.read_bytes()) == None

    req.write_bytes(packb({"query": "INSERT INTO 'test-table' (x, y) VALUES (?, ?)", "params": [30, 40], "mode": "w+"}))
    assert server.handle(str(req), str(res)) == 200
    assert unpackb(res.read_bytes()) == None

    req.write_bytes(packb({"query": "SELECT * FROM 'test-table' ORDER BY rowid", "params": [], "mode": "r"}))
    assert server.handle(str(req), str(res)) == 200
    assert unpackb(res.read_bytes()) == {"columns": ["x", "y"], "records": [{"x": 10, "y": 20}, {"x": 30, "y": 40}]}

def test_readonly_write_error():
    req.write_bytes(packb({"query": "CREATE TABLE 'readonly-error' (x)", "params": [], "mode": "r"}))
    assert server.handle(str(req), str(res)) == 400
    assert "attempt to write a readonly database" in res.read_text()

def test_invalid_query():
    req.write_bytes(packb({"query": "CREATE TABLE 'query-error'", "params": [1], "mode": "w+"}))
    assert server.handle(str(req), str(res)) == 400
    assert "Query: CREATE TABLE 'query-error'\nParams: [1]" in res.read_text()

def test_int64():
    req.write_bytes(packb({"query": "SELECT ? as value1, ? as value2", "params": [2**63 - 2, 2**63 - 1], "mode": "r"}))
    assert server.handle(str(req), str(res)) == 200
    assert unpackb(res.read_bytes()) == {"columns": ["value1", "value2"], "records": [{"value1": 2**63 - 2, "value2": 2**63 - 1}]}

def test_int_and_real():
    req.write_bytes(packb({"query": "SELECT ? as value1, ? as value2", "params": [10, 10.0], "mode": "r"}))
    assert server.handle(str(req), str(res)) == 200
    record = unpackb(res.read_bytes())["records"][0]
    assert type(record["value1"]) == int
    assert type(record["value2"]) == float

def test_blob():
    value = bytes([0, 2, 4, 8, 16, 32, 64, 128, 255])
    req.write_bytes(packb({"query": "SELECT ? as value", "params": [value], "mode": "r"}))
    assert server.handle(str(req), str(res)) == 200
    response = unpackb(res.read_bytes())["records"][0]["value"]
    assert response == value
    assert type(response) == bytes
