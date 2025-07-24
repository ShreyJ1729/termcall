import os
import shutil


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
