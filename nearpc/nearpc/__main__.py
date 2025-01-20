from openai import Client
from termcolor import colored

from . import NearPC

SYSTEM_MESSAGE = "You are a helpful and funny assistant. On your answers you try to be correct, and say some comments that make people smile."


def show_channel_info(nearpc: NearPC):
    print(colored(f"\n\nChannel ID: {nearpc.channel_id}", attrs=["bold"]))
    print(colored(f"Balance: {nearpc.balance()} NEAR", attrs=["bold"]))


def prepare_prompt(conversation) -> str:
    return (
        "\n\n".join(f'{msg["role"]}: {msg["content"]}' for msg in conversation)
        + "\n\nassistant: "
    )


def main():
    nearpc = NearPC()
    client = Client(base_url=f"{nearpc.provider_url}/oai/", api_key="n/a")

    conversation = [
        {"role": "system", "content": SYSTEM_MESSAGE},
    ]

    while True:
        show_channel_info(nearpc)

        user_input = input(
            colored("\n\n[q: quit] You >>> ", "green", attrs=["bold"])
        ).strip(" ")

        if user_input.lower() == "q":
            break

        conversation.append({"role": "user", "content": user_input})

        response = client.completions.create(
            model="fireworks::accounts/fireworks/models/llama-v3p3-70b-instruct",
            prompt=prepare_prompt(conversation),
            max_tokens=128,
            extra_headers=nearpc.make_header("0.001"),
        )

        answer = response.choices[0].text

        print(colored("\n\nAssistant >>>", "blue", attrs=["bold"]), f"{answer}")
        conversation.append({"role": "assistant", "content": answer})


if __name__ == "__main__":
    main()
