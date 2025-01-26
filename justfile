default:
  @just --list

project := justfile_directory()
tables := project + "/crates/tables"
import-from-csv := project + "/bin/import"

clippy:
  @just tables clippy
  @just import clippy
  cargo clippy --fix

clippy-lint:
  @just tables clippy-lint
  @just import clippy-lint
  cargo clippy -- -D warnings

fmt:
  @just tables fmt
  @just import fmt
  cargo fmt

fmt-check:
  @just tables fmt-check
  @just import fmt-check
  cargo fmt -- --check

check:
  @just tables check
  @just import check
  cargo check

test:
  @just tables test
  cargo test

clean:
  cargo clean

lint: check fmt clippy test

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

@tables *cmd:
  cd {{ tables }} && just {{ cmd }}

@import *cmd:
  cd {{ import-from-csv }} && just {{ cmd }}
