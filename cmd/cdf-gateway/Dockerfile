FROM golang:1.22-alpine AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY cmd/cdf-gateway ./cmd/cdf-gateway
RUN go build -o cdf-gateway ./cmd/cdf-gateway

FROM alpine:3.19
RUN apk --no-cache add ca-certificates
WORKDIR /root/
COPY --from=builder /app/cdf-gateway .
EXPOSE 8080
CMD ["./cdf-gateway"]
