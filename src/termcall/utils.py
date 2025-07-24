import os
import json
import time
import numpy as np
import cv2

CACHE_FILE = os.path.expanduser("~/.termcall/cache.json")


# Ensure cache directory exists
def _ensure_cache_dir():
    cache_dir = os.path.dirname(CACHE_FILE)
    if not os.path.exists(cache_dir):
        os.makedirs(cache_dir)


# Set a cache value with expiration (ttl in seconds)
def cache_set(key, value, ttl_seconds):
    _ensure_cache_dir()
    cache = {}
    if os.path.exists(CACHE_FILE):
        with open(CACHE_FILE, "r") as f:
            try:
                cache = json.load(f)
            except Exception:
                cache = {}
    expires_at = int(time.time()) + ttl_seconds
    cache[key] = {"value": value, "expires_at": expires_at}
    with open(CACHE_FILE, "w") as f:
        json.dump(cache, f)


# Get a cache value if not expired
def cache_get(key):
    if not os.path.exists(CACHE_FILE):
        return None
    with open(CACHE_FILE, "r") as f:
        try:
            cache = json.load(f)
        except Exception:
            return None
    entry = cache.get(key)
    if not entry:
        return None
    if int(time.time()) > entry["expires_at"]:
        cache_clear(key)
        return None
    return entry["value"]


# Clear a cache value
def cache_clear(key):
    if not os.path.exists(CACHE_FILE):
        return
    with open(CACHE_FILE, "r") as f:
        try:
            cache = json.load(f)
        except Exception:
            return
    if key in cache:
        del cache[key]
        with open(CACHE_FILE, "w") as f:
            json.dump(cache, f)


def filter_user_profiles(profiles, query):
    """Return user profiles matching the query (case-insensitive substring match on email or full_name)."""
    if not query:
        return profiles
    query = query.lower()
    return [
        p
        for p in profiles
        if query in p.get("email", "").lower()
        or query in p.get("full_name", "").lower()
    ]


def paginate_profiles(profiles, page, page_size):
    """Return a slice of user profiles for the given page and page size, and total number of pages."""
    total = len(profiles)
    if page_size <= 0:
        return profiles, 1
    total_pages = (total + page_size - 1) // page_size
    start = (page - 1) * page_size
    end = start + page_size
    return profiles[start:end], total_pages


def get_profiles_offline_first(id_token, cache_key, ttl):
    """Try to load user profiles from cache, else fetch from RTDB and update cache. Enables offline browsing."""
    profiles = cache_get(cache_key)
    if profiles is not None:
        return profiles
    from .firebase import get_all_user_profiles

    profiles = get_all_user_profiles(id_token)
    cache_set(cache_key, profiles, ttl)
    return profiles


def resize_frame(frame, target_width, target_height):
    """
    Resize an aiortc VideoFrame to (target_width, target_height) using OpenCV.
    Preserves aspect ratio by center-cropping if needed.
    Returns the resized numpy array (RGB).
    """
    # Convert VideoFrame to numpy array (RGB)
    img = frame.to_ndarray(format="rgb24")
    h, w, _ = img.shape
    # Compute aspect ratios
    src_ar = w / h
    tgt_ar = target_width / target_height
    # Center-crop to match target aspect ratio
    if src_ar > tgt_ar:
        # Source is wider: crop width
        new_w = int(h * tgt_ar)
        x0 = (w - new_w) // 2
        img_cropped = img[:, x0 : x0 + new_w, :]
    elif src_ar < tgt_ar:
        # Source is taller: crop height
        new_h = int(w / tgt_ar)
        y0 = (h - new_h) // 2
        img_cropped = img[y0 : y0 + new_h, :, :]
    else:
        img_cropped = img
    # Resize to target size
    img_resized = cv2.resize(
        img_cropped, (target_width, target_height), interpolation=cv2.INTER_LINEAR
    )
    return img_resized


def rgb_to_256color(img):
    """
    Convert an RGB numpy array (H, W, 3) to a 2D array of 256-color terminal palette indices.
    Uses xterm 256-color quantization.
    """

    # Build xterm 256-color palette
    def build_palette():
        palette = []
        # 16 basic colors
        palette += [
            (0, 0, 0),
            (128, 0, 0),
            (0, 128, 0),
            (128, 128, 0),
            (0, 0, 128),
            (128, 0, 128),
            (0, 128, 128),
            (192, 192, 192),
            (128, 128, 128),
            (255, 0, 0),
            (0, 255, 0),
            (255, 255, 0),
            (0, 0, 255),
            (255, 0, 255),
            (0, 255, 255),
            (255, 255, 255),
        ]
        # 6x6x6 color cube
        for r in range(6):
            for g in range(6):
                for b in range(6):
                    palette.append((r * 51, g * 51, b * 51))
        # 24 grayscale
        for i in range(24):
            v = 8 + i * 10
            palette.append((v, v, v))
        return np.array(palette, dtype=np.uint8)

    palette = build_palette()
    # Flatten image for vectorized distance computation
    flat = img.reshape(-1, 3).astype(np.int16)
    # Compute squared distance to each palette color
    dists = np.sum((flat[:, None, :] - palette[None, :, :]) ** 2, axis=2)
    idx = np.argmin(dists, axis=1)
    return idx.reshape(img.shape[0], img.shape[1])


def rgb_to_truecolor(img):
    """
    Pass-through for truecolor terminals. Returns the RGB numpy array as-is.
    """
    return img
