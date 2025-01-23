ls:
  just -l

clean:
  cargo clean

dev:
  cargo run

build:
  cargo build --release

deploy:
  fly deploy

secrets:
  fly secrets import < .env

logs:
  fly logs

geni direction:
  DATABASE_URL="sqlite://./local.sqlite" geni {{ direction }}
