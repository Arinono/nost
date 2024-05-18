ls:
  just -l

clean:
  cargo clean

whproxy:
  cargo run --bin whproxy

build:
  cargo build --release --all-targets

deploy-whproxy:
  fly deploy -c fly.whproxy.toml

secrets-whproxy:
  fly secrets -c fly.whproxy.toml import < .env

logs-whproxy:
  fly -c fly.whproxy.toml logs
