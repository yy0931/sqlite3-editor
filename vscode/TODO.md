sqlite3 command backend, samething like:
TODO: handle cases where `PRAGMA encoding` != 'UTF-8'

```shell
$ sqlite3 -json samples/test.db
sqlite> select (case typeof(x) when 'blob' then quote(x) when 'text' then quote(cast(x as blob)) else x end) as x_value, typeof(x) as x_type from test2;
[{"x_value":1,"x_type":"integer"},
{"x_value":1.0,"x_type":"real"},
{"x_value":"X'61'","x_type":"text"},
{"x_value":"X'11111111'","x_type":"blob"},
{"x_value":null,"x_type":"null"},
{"x_value":"X'1111'","x_type":"text"},
{"x_value":"X'00000001'","x_type":"text"}]
```
