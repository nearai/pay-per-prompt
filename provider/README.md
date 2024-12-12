# Running the provider

Ideally you should run this repo in the provided dev docker container.

make sure the following command exists:

```
$ openapi-generator-cli version
7.10.0
```

Build / run the provider:

```
./scripts/build_stubs.sh
cargo build
cargo run -- run --config ./configs/fireworks_config.yaml
```