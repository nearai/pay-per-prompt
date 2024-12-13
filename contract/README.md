# Payment channel NEAR smart contract

This contract allows establishing one directional payment channels between two users.

## Build

```
./build.sh
```

## Architecture

There are two users on a payment channel, the receiver (the one that is offering a service) and the sender (the one that is paying for the service).

## Specification

```python

ChannelId = u128

class Contract:
    # List of valid public keys for each account
    keys: Dict[AccountId, Set[PublicKey]]
    channels: Dict[ChannelId, Channel]

    def register(predecessor_account_id: AccountId, public_key: PublicKey):
        """
        Add public key to the list of valid public keys
        for the `predecessor_account_id`.

        Keys can be added to this list but can't be removed.
        """

    def open_payment_channel(
        predecessor_account_id: AccountId,
        attached_balance: Balance,
        receipient_account_id: AccountId,
    ) -> ChannelId:
        """
        Create a payment channel between `predecessor_account_id` and `receipient_account_id`,
        and attach `attached_balance` to the channel.
        """

    def withdraw(channel_id: ChannelId, state: SignedState):
        """
        Withdraw extra spent balance by the sender from the channel to the receiver.
        The state must be signed by the sender.

        Check the difference between state.channel.spent_balance and channel.withdrawn_balance
        and send the difference to the receipient, and update the channel state.
        """

    def topup(channel_id: ChannelId, attached_balance: Balance):
        """
        Add `attached_balance` to the channel balance.
        """

    def close(channel_id: ChannelId, state: SignedState):
        """
        Close the channel and send the remaining balance to the receiver.
        The state must be signed by the receiver.
        All the remaining balance is sent to the sender.
        """

    def start_hard_close(channel_id: ChannelId, predecessor_account_id: AccountId):
        """
        The sender can start a hard close of the channel. Marks the channel to be closed
        after a certain period of time. During this time the receiver can withdraw from the
        channel.
        """

    def hard_close(channel_id: ChannelId):
        """
        Close the channel after the hard close period finishes.
        """


class Channel:
    receipient: AccountId
    sender: AccountId
    added_balance: Balance
    withdrawn_balance: Balance
    start_hard_close: Option[Time]


class ChannelState:
    id: ChannelId
    spent_balance: Balance


class SignedState:
    channel: ChannelState
    # signature of the serialized channel state
    signature: Signature
    # specify the public key that was used to sign the state.
    # required since a single account id can have multiple public keys
    # associated with it.
    public_key: PublicKey
```
