# Multi-stage build: build Svelte UI, then run Rusty Gun with Deno

FROM node:20-bullseye AS webbuild
WORKDIR /app/web
COPY web/svelte/ ./svelte/
RUN cd svelte && npm i --no-fund --no-audit && npm run build

FROM denoland/deno:alpine-2.4.2
WORKDIR /app
COPY . .
COPY --from=webbuild /app/web/dist ./web/dist

# Default ports: 8080 (ws) and 8081 (http ui/api)
EXPOSE 8080 8081

CMD ["run", "-A", "--unstable-kv", "src/main.ts", "serve", "--port", "8080"]

