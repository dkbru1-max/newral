FROM rust:1.81-bookworm AS builder
# Build the release binary.
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
# Minimal runtime image.
RUN useradd -m -u 10001 app
WORKDIR /app
COPY --from=builder /app/target/release/validator-service /app/validator-service
EXPOSE 8080
USER app
ENV PORT=8080
CMD ["/app/validator-service"]
