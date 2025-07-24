"""
Firebase RTDB user schema and user creation utility.

Schema fields:
- email: str (user's email address)
- full_name: str (user's full name)
- auth_key: str (encrypted authentication key)
- last_active: int (timestamp, seconds since epoch)
"""

import time


def get_user_schema(email: str, full_name: str, auth_key: str) -> dict:
    """Return a user dict matching the Firebase RTDB schema."""
    return {
        "email": email,
        "full_name": full_name,
        "auth_key": auth_key,  # Should be encrypted before storage
        "last_active": int(time.time()),
    }
