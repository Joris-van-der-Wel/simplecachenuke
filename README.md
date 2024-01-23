# simplecachenuker

Simple daemon which clears cache directories and restarts systemd 
services after receiving a HTTP request.

Options:
```
--port <PORT>        Port to listen on
--service <SERVICE>  Name of a systemd service to stop and start. 
                     May be specified multiple times
--path <PATH>        Glob pattern to match files to delete while the 
                     service(s) have been stopped. Directories are 
                     deleted recursively. May be specified multiple
                     times
-h, --help           Print help
-V, --version        Print version
```

Example:
```
simplecachenuker --port 1234 --path '/var/cache/foo/*' --service foo.service
```

This will start the daemon on port 1234. A POST request can then be
sent to http://localhost:1234/ with a JSON request body. This will
trigger the cache nuke. All files, directories, symlinks beneath
/var/cache/foo/ will be removed, after which the foo service will be
restarted.

An empty JSON document "{}" will trigger the cache nuke immediately.
By specifying the "delay" property, the trigger will be delayed by the
given amount of seconds. Any further cache nuke requests will be
combined until the delay is over. This makes it easy to implement
"debouncing": Simply send a cache nuke request for every change to the
content, and the delay property will make sure the cache is cleared
only sparingly.

Example:
```
curl -X POST -H "Content-Type: application/json" --data '{"delay": 10}' http://localhost:1234/
```

## Why?
I wrote this in an afternoon as a quick workaround for an existing 
caching setup on a local server that no longer supports 
programmatically purging the cache.

I used rust to simplify the deployment. The tool is compiled into a 
single self contained binary.
