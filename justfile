clean:
  cargo clean

whproxy:
  cargo run --bin whproxy

build:
  cargo build --release --all-targets

deploy-whproxy:
  fly deploy -c fly.whproxy.toml
