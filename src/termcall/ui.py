import os
import shutil
import sys
import termios
import tty


def clear_screen():
    """Clear the terminal screen."""
    os.system("cls" if os.name == "nt" else "clear")


def get_terminal_size():
    """Return (columns, rows) of the terminal window."""
    size = shutil.get_terminal_size(fallback=(80, 24))
    return size.columns, size.lines


# Placeholder for interactive UI foundation
def interactive_ui():
    """Entry point for interactive terminal UI (to be implemented)."""
    clear_screen()
    print("[Interactive UI placeholder]")


def getch():
    """Read a single character from stdin (including arrow keys)."""
    fd = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        ch = sys.stdin.read(1)
        if ch == "\x1b":  # Escape sequence
            ch += sys.stdin.read(2)
        return ch
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)


def keyboard_event_loop():
    """Basic event loop for keyboard navigation demo."""
    print("Press arrow keys, Enter, or Ctrl+C to exit.")
    while True:
        key = getch()
        if key == "\x03":  # Ctrl+C
            print("Exiting...")
            break
        elif key == "\x1b[A":
            print("Up arrow")
        elif key == "\x1b[B":
            print("Down arrow")
        elif key == "\x1b[C":
            print("Right arrow")
        elif key == "\x1b[D":
            print("Left arrow")
        elif key == "\r":
            print("Enter pressed")
        else:
            print(f"Key: {repr(key)}")


def display_user_list(users, selected_index=0, window_size=10):
    """Display a scrollable user list with selection highlighting."""
    total = len(users)
    start = max(0, selected_index - window_size // 2)
    end = min(total, start + window_size)
    start = max(0, end - window_size)  # Adjust if near end
    for i in range(start, end):
        prefix = "> " if i == selected_index else "  "
        print(f"{prefix}{users[i]}")
    print(f"\nShowing {start+1}-{end} of {total} users. Use arrow keys to scroll.")


def filter_users(users, query):
    """Return users matching the query (case-insensitive substring match)."""
    if not query:
        return users
    query = query.lower()
    return [u for u in users if query in u.lower()]


def show_error(message):
    """Print an error message in red."""
    print(f"\033[91m[ERROR]\033[0m {message}")


def show_status(message):
    """Print a status/info message in green."""
    print(f"\033[92m[INFO]\033[0m {message}")
