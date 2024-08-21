# LibHttp

## usage

```
libstd = require("/home/nbittich/toyprograms/adana-std/target/release/libadana_std_http.so")
http_server=libstd.new() # listen to 8000 by default
http_handle = libstd.start(http_server)
res =libstd.stop(http_handle)
drop(http_server)
```
