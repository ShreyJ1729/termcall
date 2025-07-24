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
