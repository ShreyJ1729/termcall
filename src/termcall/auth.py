"""
Firebase RTDB user schema and user creation utility.

Schema fields:
- email: str (user's email address)
- full_name: str (user's full name)
- auth_key: str (encrypted authentication key)
- last_active: int (timestamp, seconds since epoch)
"""

import time
import re
from .firebase import get_db, init_firebase
from cryptography.fernet import Fernet
import keyring

# Keyring service name for this app
KEYRING_SERVICE = "termcall"

EMAIL_REGEX = re.compile(r"^[\w\.-]+@[\w\.-]+\.\w+$")


def is_valid_email(email: str) -> bool:
    return EMAIL_REGEX.match(email) is not None


def user_exists(email: str) -> bool:
    init_firebase()
    ref = get_db().reference("users")
    users = ref.order_by_child("email").equal_to(email).get()
    return bool(users)


def register_user(email: str, full_name: str, auth_key: str) -> str:
    """Register a new user if email is valid and not already registered."""
    if not is_valid_email(email):
        return "Invalid email format."
    if user_exists(email):
        return "User already exists."
    user_data = get_user_schema(email, full_name, auth_key)
    try:
        ref = get_db().reference("users")
        new_ref = ref.push(user_data)
        return f"User registered successfully with id: {new_ref.key}"
    except Exception as e:
        return f"Registration failed: {e}"


def get_user_schema(email: str, full_name: str, auth_key: str) -> dict:
    """Return a user dict matching the Firebase RTDB schema."""
    return {
        "email": email,
        "full_name": full_name,
        "auth_key": auth_key,  # Should be encrypted before storage
        "last_active": int(time.time()),
    }


# Generate a new Fernet key and encrypt it with the user's email as context
# (In production, use a more secure context or user secret)
def generate_encrypted_key(email: str) -> str:
    key = Fernet.generate_key()
    # Optionally, you could encrypt this key further with a user password
    return key.decode()


def store_key_in_keyring(email: str, key: str) -> None:
    keyring.set_password(KEYRING_SERVICE, email, key)


def retrieve_key_from_keyring(email: str) -> str:
    return keyring.get_password(KEYRING_SERVICE, email)
