import click


@click.group()
@click.version_option("0.1.0", prog_name="TermCall")
def main():
    """TermCall CLI - Terminal-based video calls with Firebase and WebRTC."""
    pass


@main.command()
@click.argument("email")
@click.argument("password")
def login(email, password):
    """Login to your TermCall account."""
    click.echo(f"Logging in as {email} (stub)")


@main.command()
def logout():
    """Logout and clear session."""
    click.echo("Logging out (stub)")


@main.command()
def list():
    """List all users."""
    click.echo("Listing users (stub)")


@main.command()
@click.argument("query", required=False)
def search(query):
    """Search for users by email or name."""
    click.echo(f"Searching for: {query} (stub)")


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
