import json
import os
import urllib.request

from openai import OpenAI


gateway = os.environ.get("GATEWAY_URL", "http://localhost:8080")
mock = os.environ.get("MOCK_URL", "http://localhost:4010")
jwt = json.load(urllib.request.urlopen(f"{mock}/token"))["token"]


def complete(api_key):
    client = OpenAI(base_url=f"{gateway}/v1", api_key=api_key)
    chunks = client.chat.completions.create(
        model="gateway-default",
        messages=[{"role": "user", "content": "hello"}],
        stream=True,
        stream_options={"include_usage": True},
    )
    text = "".join(choice.delta.content or "" for chunk in chunks for choice in chunk.choices)
    assert text == "hello", text


complete(jwt)
request = urllib.request.Request(
    f"{gateway}/admin/virtual-keys",
    data=json.dumps({"daily_token_limit": 10_000, "scopes": ["chat"]}).encode(),
    headers={"authorization": f"Bearer {jwt}", "content-type": "application/json"},
    method="POST",
)
virtual_key = json.load(urllib.request.urlopen(request))["key"]
complete(virtual_key)
print("OpenAI SDK JWT and Virtual Key streaming workflows passed")
