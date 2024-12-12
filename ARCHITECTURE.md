# Pay per Prompt architecture

This repository contains the implementation of a one way payment channel on NEAR, and an implementation to serve LLM completions on top of it.

In particular it contains four components:

-   Contract: Near smart contract to establish the payment channel.
-   Payment Channel Cli: Command line interface for users to interact with the payment channels.
-   Provider: A server that serves LLM completions if it receives a valid payment over a payment channel.
-   SDK: Python library to for users to interact with providers using payment channels.
