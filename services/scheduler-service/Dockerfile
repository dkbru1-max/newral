FROM rust:1.88-bookworm AS builder
# Build the release binary.
WORKDIR /app
COPY services/common ./common
COPY services/scheduler-service/Cargo.toml ./scheduler-service/Cargo.toml
COPY services/scheduler-service/src ./scheduler-service/src
COPY VERSION /app/VERSION
WORKDIR /app/scheduler-service
RUN cargo build --release

FROM debian:bookworm-slim
# Minimal runtime image.
RUN useradd -m -u 10001 app
WORKDIR /app
COPY --from=builder /app/scheduler-service/target/release/scheduler-service /app/scheduler-service
COPY scripts/bpsw /app/scripts/bpsw
EXPOSE 8080
USER app
ENV PORT=8080
CMD ["/app/scheduler-service"]
