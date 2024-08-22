# LibHttp

## usage

```
http = require("/devdisk/sideprojects/adana-std/target/release/libadana_std_http.so")
http_server=http.new() # listen to 8000 by default
settings = struct {
   static: struct {},
   middlewares: [
      struct {
      	path: "/",
      	handler: (req) => {
      	    println(req)
      	    "hello bro!"
      	},
        method: "GET"
      }
   ]
}
http_handle = http.start(http_server, settings, struct {})
res =http.stop(http_handle)
drop(http_server)
```
