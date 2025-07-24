import os
import json
import time

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
