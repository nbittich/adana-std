# LibHttp

## usage

```
http = require("/devdisk/sideprojects/adana-std/target/release/libadana_std_http.so")
http_server=http.new() # listen to 8000 by default
settings = struct {
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
      	path: "/hello/:name",
      	handler: (req) => {
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
      	handler: (req) => {
            println(req)
      	    return "hello bro!"
      },
        method: "GET"
      }
   ]
}
http_handle = http.start(http_server, settings, struct {})
res =http.stop(http_handle)
drop(http_server)
```
