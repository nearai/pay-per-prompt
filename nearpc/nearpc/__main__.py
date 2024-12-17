from . import NearPC
from openai import Client


def main():
    nearpc = NearPC()
    client = Client(base_url=nearpc.provider_url, api_key="n/a")

    # TODO: Delete me
    # When `update` is False, the local version of the channel is not updated
    header = nearpc.make_header("0.0001", update=False)
    print(header)

    client.chat.completions.create(
        model="llama3",
        messages=[{"role": "user", "content": "What are payment channels?"}],
        extra_headers={"NEAR_PC_CHANNEL": nearpc.make_header("0.0001")},
    )


if __name__ == "__main__":
    main()
