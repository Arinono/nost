FROM rust:1.78 as builder

WORKDIR /usr/src/app

COPY ../.. .

RUN apt-get update && apt install -y openssl ca-certificates curl && update-ca-certificates

RUN cargo build --release

##############################################

FROM debian:bookworm-slim

WORKDIR /app

COPY --from=builder /usr/src/app/target/release/nost /app/bin

RUN apt-get update && apt install -y openssl ca-certificates curl

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

CMD ["/app/bin"]

