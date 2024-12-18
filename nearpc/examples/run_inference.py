#!/usr/bin/env python3
from nearpc import NearPC
from openai import Client

base_url = "http://localhost:9999"

nearpc = NearPC(provider_url=base_url)
client = Client(base_url=f"{base_url}/oai/", api_key="n/a")
print(f"Remaining balance: {nearpc.balance()} NEAR")

# When `update` is False, the local version of the channel is not updated
prompt = input("Enter prompt: ")
response = client.completions.create(
  model="fireworks::accounts/fireworks/models/llama-v3p3-70b-instruct",
  prompt=prompt,
  max_tokens=128,
  extra_headers=nearpc.make_header(nearpc.spent() + 0.001, update=False),
)
print(response.choices[0].text)
print(f"Remaining balance: {nearpc.balance()} NEAR")
