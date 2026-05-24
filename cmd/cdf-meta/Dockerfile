FROM golang:1.22-alpine AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY cmd/cdf-meta ./cmd/cdf-meta
RUN go build -o cdf-meta ./cmd/cdf-meta

FROM alpine:latest
RUN apk --no-cache add ca-certificates
WORKDIR /root/
COPY --from=builder /app/cdf-meta .
EXPOSE 50054
CMD ["./cdf-meta"]
