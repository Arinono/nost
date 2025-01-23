ls:
  just -l

clean:
  cargo clean

dev:
  cargo run

build:
  cargo build --release

deploy-whproxy:
  fly deploy

secrets-whproxy:
  fly secrets import < .env

logs-whproxy:
  fly logs
