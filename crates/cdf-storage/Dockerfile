FROM rust:1.75-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/cdf-common crates/cdf-common
COPY crates/cdf-storage crates/cdf-storage
RUN cargo build --release -p cdf-storage

FROM alpine:latest
RUN apk --no-cache add ca-certificates
WORKDIR /root/
COPY --from=builder /app/target/release/cdf-storage .
EXPOSE 50051
CMD ["./cdf-storage"]
