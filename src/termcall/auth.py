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
from .firebase import get_firebase
from cryptography.fernet import Fernet
import keyring
import json
import os

# Keyring service name for this app
KEYRING_SERVICE = "termcall"

EMAIL_REGEX = re.compile(r"^[\w\.-]+@[\w\.-]+\.\w+$")

SESSION_FILE = os.path.expanduser("~/.termcall_session")


def is_valid_email(email: str) -> bool:
    return EMAIL_REGEX.match(email) is not None


def get_user_schema(email: str, full_name: str, auth_key: str) -> dict:
    """Return a user dict matching the Firebase RTDB schema."""
    return {
        "email": email,
        "full_name": full_name,
        "auth_key": auth_key,  # Should be encrypted before storage
        "last_active": int(time.time()),
    }


def register_user(email: str, password: str, full_name: str) -> str:
    """Register a new user with Firebase Auth. Always store profile in RTDB."""
    if not is_valid_email(email):
        return "Invalid email format."
    try:
        firebase, auth, db = get_firebase()
        user = auth.create_user_with_email_and_password(email, password)
        # Always store profile data in RTDB after registration
        user_data = get_user_schema(email, full_name, "")
        db.child("users").child(user["localId"]).set(user_data, user["idToken"])
        return f"User registered successfully with id: {user['localId']}"
    except Exception as e:
        return f"Registration failed: {e}"


# Generate a new Fernet key and encrypt it with the user's email as context
# (In production, use a more secure context or user secret)
def generate_encrypted_key(email: str) -> str:
    key = Fernet.generate_key()
    return key.decode()


def store_key_in_keyring(email: str, key: str) -> None:
    keyring.set_password(KEYRING_SERVICE, email, key)


def retrieve_key_from_keyring(email: str) -> str:
    return keyring.get_password(KEYRING_SERVICE, email)


def login_user(email: str, password: str):
    """Authenticate user with Firebase Auth. Returns user dict (idToken, refreshToken, localId, etc.) or (None, error)."""
    try:
        _, auth, _ = get_firebase()
        user = auth.sign_in_with_email_and_password(email, password)
        return user  # Return the full user dict
    except Exception as e:
        return None, f"Login failed: {e}"


def save_session(
    id_token: str,
    refresh_token: str,
    local_id: str,
    email: str = None,
    full_name: str = None,
):
    data = {
        "idToken": id_token,
        "refreshToken": refresh_token,
        "localId": local_id,
    }
    if email:
        data["email"] = email
    if full_name:
        data["full_name"] = full_name
    with open(SESSION_FILE, "w") as f:
        json.dump(data, f)


def load_session():
    if not os.path.exists(SESSION_FILE):
        return None
    with open(SESSION_FILE, "r") as f:
        return json.load(f)


def validate_session():
    session = load_session()
    if not session:
        return False, None
    _, auth, _ = get_firebase()
    try:
        user = auth.refresh(session["refreshToken"])
        save_session(user["idToken"], user["refreshToken"], user["userId"])
        return True, user
    except Exception:
        return False, None
