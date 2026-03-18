# Standalone frontend container — serves Svelte SPA via nginx
FROM oven/bun:1 AS builder
WORKDIR /web
COPY package.json ./
RUN bun install
COPY . .
RUN bun run build

FROM nginx:alpine
COPY --from=builder /web/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 8080
