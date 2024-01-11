from tqdm import tqdm
import json
import os
from PIL import Image, ImageDraw, ImageFont
import numpy as np
import unicodedata


def render_character_monospace(character, font_size=50):
    """Render a character in monospace font and return the proportion of dark pixels."""
    image = Image.new("L", (font_size, font_size), color="white")
    draw = ImageDraw.Draw(image)
    # default terminal font
    font = ImageFont.truetype("Menlo-Regular.ttf", 100)

    # Centering the character in the square
    text_width, text_height = draw.textbbox((0, 0), character, font=font)[2:]
    x = (font_size - text_width) // 2
    y = (font_size - text_height) // 2
    draw.text((x, y), character, fill="black", font=font)

    # Count dark pixels
    np_image = np.array(image)
    dark_pixels = np.sum(np_image < 128)  # Threshold for dark pixels
    total_pixels = font_size * font_size

    return image, dark_pixels / total_pixels


# Dictionary to store darkness proportions for monospace font
character_darkness_monospace = {}

# Selecting all printable unicode characters for demonstration
characters = [chr(i) for i in range(0, 65536)]

for el in tqdm(enumerate(characters), total=len(characters)):
    i, char = el
    image, darkness = render_character_monospace(char)
    character_darkness_monospace[char] = darkness

mapping = character_darkness_monospace

# find the most common darkness value
from collections import Counter

darknesses = Counter(mapping.values())
most_common_darkness = darknesses.most_common(1)[0][0]

# remove all chars with most common darkness value
mapping_copy = mapping.copy()
for char, darkness in mapping_copy.items():
    if darkness == most_common_darkness:
        del mapping[char]

# create groups of chars with same darkness values
groups = {}
for char, darkness in mapping.items():
    if darkness not in groups:
        groups[darkness] = []
    groups[darkness].append(char)

# only keep the first char of every group
for darkness, chars in groups.items():
    groups[darkness] = chars[0]

# only keep those chars in the mapping
mapping_copy = mapping.copy()
for char, darkness in mapping_copy.items():
    if char not in groups.values():
        del mapping[char]

# assert that all chars have unique darkness values
assert len(set(mapping.values())) == len(mapping)

# save to json
with open("mapping.json", "w+") as f:
    json.dump(character_darkness_monospace, f)
