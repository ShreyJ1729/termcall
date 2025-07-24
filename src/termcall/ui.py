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
