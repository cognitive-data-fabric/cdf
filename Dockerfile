# CDF Gateway — Multi-stage Dockerfile
# Builds the Go gateway service for production deployment
# Usage:
#   docker build -t cdf-gateway .
#   docker run -p 8080:8080 cdf-gateway

# --- Build Stage ---
FROM golang:1.22-alpine AS builder
WORKDIR /build
COPY go.mod go.sum ./
RUN go mod download
COPY cmd/ cmd/
RUN CGO_ENABLED=0 GOOS=linux go build -o /cdf-gateway ./cmd/cdf-gateway

# --- Production Stage ---
FROM alpine:3.19 AS prod
RUN apk add --no-cache ca-certificates
COPY --from=builder /cdf-gateway /usr/local/bin/cdf-gateway
EXPOSE 8080 50053
ENTRYPOINT ["cdf-gateway"]
