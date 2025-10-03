# Multi-stage build: build Svelte UI, then run PluresDB with Deno

FROM node:20-bullseye AS webbuild
WORKDIR /app/web
COPY web/svelte/ ./svelte/
RUN cd svelte && npm i --no-fund --no-audit && npm run build

FROM denoland/deno:alpine-2.4.2
WORKDIR /app
COPY . .
COPY --from=webbuild /app/web/dist ./web/dist

# Default ports: 34567 (API) and 34568 (Web UI)
EXPOSE 34567 34568

CMD ["run", "-A", "--unstable-kv", "src/main.ts", "serve", "--port", "34567", "--web-port", "34568"]

