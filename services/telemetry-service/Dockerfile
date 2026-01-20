ARG REGISTRY_PREFIX=docker.io/
FROM ${REGISTRY_PREFIX}library/rust:1.88-bookworm AS builder
# Build the release binary.
WORKDIR /app
COPY services/common ./common
COPY services/telemetry-service/Cargo.toml ./telemetry-service/Cargo.toml
COPY services/telemetry-service/src ./telemetry-service/src
WORKDIR /app/telemetry-service
RUN cargo build --release

FROM ${REGISTRY_PREFIX}library/debian:bookworm-slim
# Minimal runtime image.
RUN useradd -m -u 10001 app && mkdir -p /app/logs && chown -R app:app /app/logs
WORKDIR /app
COPY --from=builder /app/telemetry-service/target/release/telemetry-service /app/telemetry-service
EXPOSE 8080
USER app
ENV PORT=8080
CMD ["/app/telemetry-service"]
