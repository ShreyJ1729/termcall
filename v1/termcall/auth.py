import json
import uuid
from pathlib import Path

TOKEN_PATH = Path.home() / ".termcall" / "token.json"


def get_local_token():
    if TOKEN_PATH.exists():
        with open(TOKEN_PATH, "r") as f:
            return json.load(f)
    return None


def save_local_token(email, token=None):
    TOKEN_PATH.parent.mkdir(exist_ok=True)
    with open(TOKEN_PATH, "w") as f:
        json.dump({"email": email}, f)


async def authenticate():
    from prompt_toolkit import PromptSession

    data = get_local_token()
    if data:
        email = data["email"]
        print(f"Authenticated as {email}")
        return email, email
    session = PromptSession()
    email = await session.prompt_async("Email: ")
    full_name = await session.prompt_async("Full Name: ")
    save_local_token(email)
    print(f"Authenticated as {full_name} ({email})")
    return email, full_name


async def view_profile(email):
    print("\n--- Profile ---")
    print(f"Email: {email}")
    print("--------------\n")
    import asyncio

    await asyncio.sleep(0.5)
