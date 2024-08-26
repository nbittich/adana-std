# LibHttp

## usage

```
http = require("/devdisk/sideprojects/adana-std/target/release/libadana_std_http.so")
http_server=http.new() # listen to 8000 by default
ctx = struct {}
settings = struct {
   store: struct {todos: []},
   static: [
      struct {
         path: "/favicon.ico",
         file_path: "/devdisk/sideprojects/openartcoded/monorepo/backoffice/src/favicon.ico"
      },
      struct {
         path: "/playground",
         file_path: "/devdisk/sideprojects/adana-playground"
      }
   ],
   middlewares: [

      struct {
      	path: "/todo",
      	handler: (req, store) => {
             store.todos += [struct {todo: req.form.todo}]
      	    return struct {
              status: 200,
              body: struct { response: store.todos },
              headers: struct { "Content-Type": "application/json"}
            }
      	},
        method: "POST"
      },
      struct {
      	path: "/hello/:name",
      	handler: (req, store) => {
            println(req)
      	    return struct {
              status: 200,
              body: struct { response: """hello ${req.params.name}!""" },
              headers: struct { "Content-Type": "application/json"}
            }
      	},
        method: "GET"
      },
      struct {
      	path: "/",
      	handler: (req, store) => {
            println(req)
      	    return "hello bro!"
      },
        method: "GET"
      }
   ]
}
http_handle = http.start(http_server, settings, ctx)
res =http.stop(http_handle)
drop(http_server)
```

### curl

`curl -X POST http://localhost:8000/todo      -H "Content-Type: application/x-www-form-urlencoded"      -d "todo=Hello bro"`
