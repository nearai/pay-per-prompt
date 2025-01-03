# Pay per Prompt

Payment channel infrastructure to connect with LLM providers, and pay per prompt.

## Description

This project aims to provide all the necessary components and infrastructure to setup payment channels between users and LLM providers. Users pay for LLM completions from their NEAR accounts using the familiar `openai` SDK to do so.

## Components

- `provider`: An OpenAI API compatible proxy server with middleware to validate and maintain payment channels message headers.
- `contract`: The NEAR smart contract that manages payment channels.
- `cli`: A command line interface for interacting with the payment channel smart contract and provider. It allows users to create, topup, withdraw, and close payment channels.
- `nearpc`: A Python library that allows users to generate the right HTTP payment channel headers to pay for their requests. Users use this with the `openai` sdk.

## Examples

### Pay for completion service

1. (optional) Start a self hosted provider

    ```sh
    cd provider
    cargo run -- --config <config-file>
    ```

2. Create a new channel via the cli (adjust base amount as needed)

    ```sh
    cd cli
    cargo run -- open '0.01 NEAR'
    ```

3. Send requests to the provider (paying as you go)

    ```sh
    cd nearpc
    poetry run ./examples/run_inference.py
    ```

4. Check spent balance from the provider via the cli or curl

    ```sh
    cd cli
    cargo run -- info <channel-id>
    ```

    or

    ```sh
    curl http://<provider-url>/pc/state/<channel-id>
    ```

5. Close the channel

    ```sh
    cd cli
    cargo run -- close <channel-id>
    ```

### Validating a signed state

A signed state is a base64 encoded borsh serialized `SignedState` struct. This payload contains the amount the user has / wants to spend with a signature to verify it's being sent from the user.

1. To create a signed state payload you can do:

    ```sh
    cd cli
    cargo run -- advanced send '0.0001 NEAR' <channel-id>
    ```

2. Use curl to validate the signed state with a provider (expect 200 or else adjust payload amount as needed)

    ```sh
    $ curl -v -X POST http://<provider-url>/pc/validate \
        -H "Content-Type: application/json" \
        --data-raw '<payload-from-nearpc>'
    ```
