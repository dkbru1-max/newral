FROM rust:1.88-bookworm AS builder
# Build the release binary.
WORKDIR /app
COPY services/common ./common
COPY services/validator-service/Cargo.toml ./validator-service/Cargo.toml
COPY services/validator-service/src ./validator-service/src
WORKDIR /app/validator-service
RUN cargo build --release

FROM debian:bookworm-slim
# Minimal runtime image.
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3-minimal \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 app \
    && mkdir -p /app/logs \
    && chown -R app:app /app/logs
WORKDIR /app
COPY --from=builder /app/validator-service/target/release/validator-service /app/validator-service
EXPOSE 8080
USER app
ENV PORT=8080
CMD ["/app/validator-service"]
