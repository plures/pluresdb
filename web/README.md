# Web UI (Svelte)

This folder contains a Svelte-based reactive UI for PluresDB.

- svelte/ source using Vite
- Built assets go to web/dist/ and are served by the PluresDB HTTP server

## Dev

Use Node 18+.

```
cd web/svelte
npm i
npm run dev
```

Configure the API base URL if needed (defaults to same origin).

## Build

```
cd web/svelte
npm run build
```

This outputs to ../dist/. Start PluresDB serve and open the port+1 URL printed in the console.






