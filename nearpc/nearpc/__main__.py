from . import NearPC
from openai import Client


def main():
    nearpc = NearPC()
    client = Client(base_url=f"{nearpc.provider_url}/oai/", api_key="n/a")

    response = client.completions.create(
        model="fireworks::accounts/fireworks/models/llama-v3p3-70b-instruct",
        prompt="What are payment channels?",
        max_tokens=128,
        extra_headers=nearpc.make_header("0.001"),
    )

    print(response.model_dump_json(indent=2))


if __name__ == "__main__":
    main()
