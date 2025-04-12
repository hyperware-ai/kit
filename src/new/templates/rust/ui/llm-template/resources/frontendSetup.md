# Frontend/UI Development

Hyperware can easily serve any webpage or web app developed with normal libraries and frameworks.

There are some specific endpoints, JS libraries, and `process_lib` functions that are helpful for doing frontend development.

There are also some important considerations and "gotchas" that can happen when trying to do frontend development.

Hyperware can serve a website or web app just like any HTTP webserver.
The preferred method is to upload your static assets on install by placing them in the `pkg` folder.
By convention, `kit` bundles these assets into a directory inside `pkg` called `ui`, but you can call it anything.
You **must** place your `index.html` in the top-level folder.
The structure should look like this:

```
my-package
└── pkg
    └── ui (can have any name)
        ├── assets (can have any name)
        └── index.html
```

## /our & /our.js

Every node has both `/our` and `/our.js` endpoints.
`/our` returns the node's ID as a string like `'my-node'`.
`/our.js` returns a JS script that sets `window.our = { node: 'my-node' }`.
By convention, you can then easily set `window.our.process` either in your UI code or from a process-specific endpoint.
The frontend would then have `window.our` set for use in your code.

## Serving a Website

The simplest way to serve a UI is using the `serve_ui` function from `process_lib`:

```
serve_ui(&our, "ui", true, false, vec!["/"]).unwrap();
```

This will serve the `index.html` in the specified folder (here, `"ui"`) at the home path of your process.
If your process is called `my-process:my-package:template.os` and your Hyperware node is running locally on port 8080,
then the UI will be served at `http://localhost:8080/my-process:my-package:template.os`.

`serve_ui` takes five arguments: our `&Address`, the name of the folder that contains your frontend, whether the UI requires authentication, whether the UI is local-only, and the path(s) on which to serve the UI (usually `["/"]`).

## Development without kit

The `kit` UI template uses the React framework compiled with Vite.
But you can use any UI framework as long as it generates an `index.html` and associated assets.
To make development easy, your setup should support a base URL and http proxying.

### Base URL

All processes on Hyperware are namespaced by process name in the standard format of `process:package:publisher`.
So if your process is called `my-process:my-package:template.os`, then your process can only bind HTTP paths that start with `/my-process:my-package:template.os`.
Your UI should be developed and compiled with the base URL set to the appropriate process path.

#### Vite

In `vite.config.ts` (or `.js`) set `base` to your full process name, i.e.
```
base: '/my-process:my-package:template.os'
```

#### Create React App

In `package.json` set `homepage` to your full process name, i.e.
```
homepage: '/my-process:my-package:template.os'
```

### Proxying HTTP Requests

In UI development, it is very useful to proxy HTTP requests from the in-dev UI to your Hyperware node.
Below are some examples.

#### Vite

Follow the `server` entry in the [kit template](https://github.com/hyperware-ai/kit/blob/master/src/new/templates/ui/chat/ui/vite.config.ts#L31-L47) in your own `vite.config.ts`.

#### Create React App

In `package.json` set `proxy` to your nodes URL, i.e.
```
proxy: 'http://localhost:8080'
```

### Making HTTP Requests

When making HTTP requests in your UI, make sure to prepend your base URL to the request.
For example, if your base URL is `/my-process:my-package:template.os`, then a `fetch` request to `/my-endpoint` would look like this:

```
fetch('/my-process:my-package:template.os/my-endpoint')
```

## Local Development and "gotchas"

When developing a frontend locally, particularly with a framework like React, it is helpful to proxy HTTP requests through to your node.
The `vite.config.ts` provided in the `kit` template has code to handle this proxying.

It is important to remember that the frontend will always have the process name as the first part of the HTTP path,
so all HTTP requests and file sources should start with the process name.
Many frontend JavaScript frameworks will handle this by default if you set the `base` or `baseUrl` properly.

In development, websocket connections can be more annoying to proxy, so it is often easier to simply hardcode the URL if in development.
See your framework documentation for how to check if you are in dev or prod.
The `kit` template already handles this for you.

Developing against a remote node is simple, you just have to change the proxy target in `vite.config.ts` to the URL of your node.
By default the template will target `http://localhost:8080`.