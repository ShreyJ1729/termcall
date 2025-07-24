import click
import os
import secrets
from .auth import (
    validate_session,
    save_session,
    load_session,
    register_user,
    login_user,
)

TERMCALL_DIR = os.path.expanduser("~/.termcall")


@click.group()
@click.version_option("0.1.0", prog_name="TermCall")
def main():
    """TermCall CLI - Terminal-based video calls with Firebase and WebRTC."""
    pass


@main.command()
@click.argument("email")
def login(email):
    """Login or register to your TermCall account (auto password)."""
    from .ui import show_status, show_error

    if not os.path.exists(TERMCALL_DIR):
        os.makedirs(TERMCALL_DIR)
    pw_file = os.path.join(TERMCALL_DIR, f"{email}.pw")
    # Check for existing session
    valid, session = validate_session()
    if valid:
        show_status(f"Already logged in as {email}.")
        return
    # Try to load password from file
    if os.path.exists(pw_file):
        with open(pw_file, "r") as f:
            password = f.read().strip()
        user = login_user(email, password)
        if user and isinstance(user, dict):
            save_session(user["idToken"], user["refreshToken"], user["localId"])
            show_status(f"Logged in as {email}.")
            return
        else:
            show_error("Login failed. Try deleting your .pw file and re-running.")
            return
    # Registration workflow
    click.echo("No account found. Registering new user...")
    full_name = click.prompt("Enter your full name")
    password = secrets.token_urlsafe(16)
    result = register_user(email, password, full_name)
    if "successfully" in result:
        with open(pw_file, "w") as f:
            f.write(password)
        user = login_user(email, password)
        if user and isinstance(user, dict):
            save_session(user["idToken"], user["refreshToken"], user["localId"])
            show_status(f"Registered and logged in as {email}.")
        else:
            show_error("Registration succeeded but login failed. Try again.")
    else:
        show_error(result)


@main.command()
def logout():
    """Logout and clear session."""
    click.echo("Logging out (stub)")


@main.command()
def list():
    """List all users (from Firebase profile directory, with cache)."""
    from .auth import load_session, get_user_schema
    from .firebase import get_firebase
    from .utils import get_profiles_offline_first
    from .ui import show_status, show_error

    session = load_session()
    if not session:
        show_error("Not logged in.")
        return
    id_token = session["idToken"]
    local_id = session["localId"]
    email = session.get("email")
    full_name = session.get("full_name", "")
    # Ensure current user's profile is present in RTDB
    _, _, db = get_firebase()
    user_profile = db.child("users").child(local_id).get(id_token).val()
    if not user_profile:
        user_data = get_user_schema(email, full_name, "")
        db.child("users").child(local_id).set(user_data, id_token)
    try:
        profiles = get_profiles_offline_first(
            id_token, "user_profiles", 300
        )  # 5 min cache
    except Exception as e:
        show_error(f"Failed to load user profiles: {e}")
        return
    if not profiles:
        show_status("No users found.")
        return
    for p in profiles:
        marker = "*" if p.get("email") and p.get("email") == email else " "
        print(f"{marker} {p.get('email', ''):30} {p.get('full_name', '')}")
    print("\n* = you")


@main.command()
@click.argument("query", required=False)
def search(query):
    """Search for users by email or name."""
    from .auth import load_session
    from .utils import get_profiles_offline_first, filter_user_profiles
    from .ui import show_status, show_error

    session = load_session()
    if not session:
        show_error("Not logged in.")
        return
    id_token = session["idToken"]
    try:
        profiles = get_profiles_offline_first(id_token, "user_profiles", 300)
    except Exception as e:
        show_error(f"Failed to load user profiles: {e}")
        return
    if not profiles:
        show_status("No users found.")
        return
    results = filter_user_profiles(profiles, query)
    if not results:
        show_status(f"No users found matching '{query}'.")
        return
    print(f"Found {len(results)} user(s) matching '{query}':\n")
    for p in results:
        print(f"{p.get('email','') :30} {p.get('full_name','')}")


@main.command()
@click.argument("user_id")
def call(user_id):
    """Initiate a call with a user."""
    click.echo(f"Calling user {user_id} (stub)")


@main.command()
def end():
    """End the current call."""
    click.echo("Ending call (stub)")


if __name__ == "__main__":
    main()
