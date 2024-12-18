import base64
import json
import os
import struct
from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path
import httpx

import base58
import nacl
import nacl.public
import nacl.signing


def data() -> Path:
    return Path.home() / ".config" / "near_payment_channel"


class NearPC:
    def __init__(
        self,
        *,
        provider_url: str | None = None,
        channel_id: str | None = None,
    ):
        if channel_id is None:
            channel_id = get_only_channel()

        if provider_url is None:
            provider_url = os.environ.get("PROVIDER_URL")

        if provider_url is None:
            provider_url = "http://payperprompt.near.ai"

        self.provider_url = provider_url
        self.channel_id = channel_id

    def make_header(self, amount: str | int | float, update: bool = True) -> str:
        """
        amount: Total to attach to the channel denominated in NEAR
        update: Whether to update the channel state after signing the new state.
        """
        channel = Channel.load(self.channel_id)
        amount_near = NearToken.parseNear(amount)
        new_spent_balance = channel.spent_balance + amount_near

        if new_spent_balance > channel.added_balance:
            raise ValueError("Insufficient balance")

        channel.spent_balance = new_spent_balance
        new_payload = channel.payload()

        if update:
            channel.update()

        header_value = base64.b64encode(new_payload).decode("utf-8")
        return {"X-Payments-Signature": header_value}


    def spent(self) -> float:
        return spent_balance(self).as_near()

    def balance(self) -> float:
        return remaining_spendable_balance(self).as_near()

def get_only_channel() -> str:
    path = data() / "channels"

    if not path.exists():
        raise ValueError(f"No channels found at {path}")

    files = os.listdir(path)
    if len(files) != 1:
        raise ValueError(f"Expected exactly one channel, found {len(files)}")

    file = files[0]
    with open(path / file) as f:
        channel = json.load(f)
        return channel["channel_id"]


@dataclass
class NearToken:
    yoctoNear: int

    def as_yocto_near(self) -> int:
        return self.yoctoNear

    def as_near(self) -> float:
        return float(self.yoctoNear / 10**24)

    @staticmethod
    def parseYoctoNear(amount: str | int) -> "NearToken":
        return NearToken(int(amount))

    @staticmethod
    def parseNear(amount: str | int | float) -> "NearToken":
        return NearToken(int(Fraction(amount) * 10**24))

    def __add__(self, other: "NearToken") -> "NearToken":
        return NearToken(self.yoctoNear + other.yoctoNear)

    def __sub__(self, other: "NearToken") -> "NearToken":
        return NearToken(self.yoctoNear - other.yoctoNear)

    def __lt__(self, other: "NearToken") -> bool:
        return self.yoctoNear < other.yoctoNear


@dataclass
class AccountDetails:
    account_id: str
    public_key: nacl.public.PublicKey


@dataclass
class Channel:
    channel_id: str
    receiver: AccountDetails
    sender: AccountDetails
    sender_secret_key: nacl.signing.SigningKey
    spent_balance: NearToken
    added_balance: NearToken
    withdrawn_balance: NearToken

    @staticmethod
    def load(channel_id: str) -> "Channel":
        path = data() / "channels" / f"{channel_id}.json"

        if not path.exists():
            raise ValueError(f"No channel found at {path}")

        with open(path) as f:
            channel = json.load(f)

        return Channel(
            channel_id=channel["channel_id"],
            receiver=AccountDetails(
                account_id=channel["receiver"]["account_id"],
                public_key=parsePublicKey(channel["receiver"]["public_key"]),
            ),
            sender=AccountDetails(
                account_id=channel["sender"]["account_id"],
                public_key=parsePublicKey(channel["sender"]["public_key"]),
            ),
            sender_secret_key=parseSigningKey(channel["sender_secret_key"]),
            spent_balance=NearToken.parseYoctoNear(channel["spent_balance"]),
            added_balance=NearToken.parseYoctoNear(channel["added_balance"]),
            withdrawn_balance=NearToken.parseYoctoNear(channel["withdrawn_balance"]),
        )

    def update(self):
        path = data() / "channels" / f"{self.channel_id}.json"
        with open(path) as f:
            channel = json.load(f)

        channel["spent_balance"] = str(self.spent_balance.as_yocto_near())

        with open(path, "w") as f:
            json.dump(channel, f)

    def raw_state(self) -> bytes:
        return b"".join(
            [
                struct.pack("I", len(self.channel_id)),
                self.channel_id.encode("utf-8"),
                struct.pack("Q", self.spent_balance.yoctoNear & 0xFFFFFFFFFFFFFFFF),
                struct.pack("Q", self.spent_balance.yoctoNear >> 64),
            ]
        )

    def signed_state(self) -> bytes:
        return bytes(self.sender_secret_key.sign(self.raw_state()))[:64]

    def payload(self):
        return self.raw_state() + b"\x00" + self.signed_state()


def parsePublicKey(key: str) -> nacl.public.PublicKey:
    assert key.startswith("ed25519:")
    key = key[len("ed25519:") :]
    return nacl.public.PublicKey(base58.b58decode(key))


def parseSigningKey(key: str) -> nacl.signing.SigningKey:
    assert key.startswith("ed25519:")
    key = key[len("ed25519:") :]
    return nacl.signing.SigningKey(base58.b58decode(key)[:32])

def remaining_spendable_balance(near_pc: NearPC) -> NearToken:
    with httpx.Client() as client:
        response = client.get(f"{near_pc.provider_url}/pc/state/{near_pc.channel_id}")
        response.raise_for_status()
        state = response.json()
        spent_balance = NearToken.parseYoctoNear(state["spent_balance"])
        added_balance = NearToken.parseYoctoNear(state["added_balance"])
        return (added_balance - spent_balance)

def spent_balance(near_pc: NearPC) -> NearToken:
    with httpx.Client() as client:
        response = client.get(f"{near_pc.provider_url}/pc/state/{near_pc.channel_id}")
        response.raise_for_status()
        state = response.json()
        spent_balance = NearToken.parseYoctoNear(state["spent_balance"])
        return spent_balance
