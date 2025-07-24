import sys
import numpy as np
from image_to_ascii import ImageToAscii
from sixel import converter
import cv2

# For ASCII: instantiate ImageToAscii(img, width=..., height=..., colored=...) per image, then call .convert()
# For Sixel: use write_sixel(img, sys.stdout, width, height)


def write_sixel(img, out=sys.stdout, width=None, height=None):
    """
    Display an image as Sixel in the terminal.
    Optionally resize to (width, height) before displaying.
    """
    if width is not None and height is not None:
        img = cv2.resize(img, (width, height))
    c = converter.SixelConverter(img)
    c.write(out)


def move_cursor(row, col):
    print(f"\033[{row};{col}H", end="")
