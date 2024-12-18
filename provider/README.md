# Running the provider

Ideally you should run this repo in the provided dev docker container.

make sure the following command exists:

# Setup

Make sure you have [rust installed](https://www.rust-lang.org/tools/install):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

and you are using a recent version:

```bash
rustup update
```

[Install openapi-generator-cli](https://openapi-generator.tech/docs/installation/):

```bash
npm install @openapitools/openapi-generator-cli -g

openapi-generator-cli version
7.10.0
```

Install required dependencies in linux:

```bash
sudo apt install build-essential cmake pkg-config libudev-dev
```

Populate your config file appropriately at `configs/provider.yaml`. See example (`configs/test.yaml`).

Make sure you create a NEAR account and it its content is in `~/.near-credentials/mainnet/<account_id>.json`.
You can create your NEAR accout using [near-cli-rs](https://github.com/near/near-cli-rs).

```bash
cargo install near-cli-rs
near account create-account fund-later use-auto-generation save-to-folder /home/setup/.near-credentials/mainnet
```

Generate stubs and build the project

```
./scripts/build_stubs.sh
cargo install sqlx-cli
cargo sqlx migrate run -D 'sqlite://db.sqlite?mode=rwc'
cargo sqlx prepare -D 'sqlite://db.sqlite?mode=rwc'
cargo build
```

Run the project

```bash
cargo run -- run --config configs/provider.yaml --host 0.0.0.0 --port 8024
```
