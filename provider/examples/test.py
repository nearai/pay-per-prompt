import time
from openai import OpenAI

start = time.time()
client = OpenAI(
    base_url="http://localhost:9999/oai/",
    api_key="n/a"
)

response = client.completions.create(
  model="fireworks::accounts/fireworks/models/llama-v3p3-70b-instruct",
  prompt="Hello, my name is",
  max_tokens=10,
  temperature=0.0,
)
print(response.model_dump_json(indent=2))
end = time.time()
print(f"Time taken: {end - start} seconds")