FROM golang:1.22-alpine AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY cmd/cdf-router ./cmd/cdf-router
RUN go build -o cdf-router ./cmd/cdf-router

FROM alpine:latest
RUN apk --no-cache add ca-certificates
WORKDIR /root/
COPY --from=builder /app/cdf-router .
EXPOSE 50050
CMD ["./cdf-router"]
