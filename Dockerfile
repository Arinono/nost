FROM rust:1.78 as rust-builder

WORKDIR /usr/src/app

COPY ../.. .

RUN apt-get update && \
      apt install -y openssl ca-certificates curl && \
      update-ca-certificates

RUN cargo build --release --bin whproxy

##############################################

FROM debian:bookworm-slim

WORKDIR /app

COPY --from=rust-builder /usr/src/app/target/release/whproxy /app/bin

RUN apt-get update && \
  apt install -y openssl ca-certificates curl

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

CMD ["/app/bin"]

