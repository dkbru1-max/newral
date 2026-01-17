FROM rust:1.88-bookworm AS builder
# Build the release binary.
WORKDIR /app
COPY services/common ./common
COPY services/identity-service/Cargo.toml ./identity-service/Cargo.toml
COPY services/identity-service/src ./identity-service/src
WORKDIR /app/identity-service
RUN cargo build --release

FROM debian:bookworm-slim
# Minimal runtime image.
RUN useradd -m -u 10001 app && mkdir -p /app/logs && chown -R app:app /app/logs
WORKDIR /app
COPY --from=builder /app/identity-service/target/release/identity-service /app/identity-service
EXPOSE 8080
USER app
ENV PORT=8080
CMD ["/app/identity-service"]
