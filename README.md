Simple test server for [hyperium/h3](https://github.com/hyperium/h3).

Serves files with HTTP/3, as well as HTTP/2 and HTTP/1.1 so you can make your browser run HTTP/3 request when it accepts `Alt-Svc: h3=":443`.

A live version is available at [h3.stammw.eu](https://h3.stammw.eu/index.html).

For the moment, only Chromium has shown to be able to send HTTP/3 requests to this endpoint. If you stumble on this repo and have the time to check if your browser / lib / tool is compatible, don't hesitate to let me know in the issues!
