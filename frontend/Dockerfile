ARG REGISTRY_PREFIX=docker.io/
FROM ${REGISTRY_PREFIX}library/node:20-bookworm AS builder
# Build the portal assets.
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install
COPY frontend ./
COPY VERSION ./public/version.txt
RUN npm run build

FROM ${REGISTRY_PREFIX}library/nginx:1.27-alpine
# Serve the compiled portal.
COPY --from=builder /app/frontend/dist /usr/share/nginx/html
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
