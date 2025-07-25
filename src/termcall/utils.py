import os
import json
import time
import numpy as np
import cv2
import asyncio
import threading
import shutil
import sys
import datetime

try:
    import sixel

    _sixel_available = True
except ImportError:
    _sixel_available = False

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


def brightness_to_ascii(img, density="high"):
    """
    Map pixel brightness to ASCII characters.
    img: 2D grayscale or 3D RGB numpy array
    density: 'high', 'medium', or 'low' (controls character set)
    Returns: 2D array of ASCII characters (same shape as input)
    """
    ramps = {"high": "@%#*+=-:. ", "medium": "@#S%=*+:-. ", "low": "@#- "}
    ramp = ramps.get(density, ramps["high"])
    n = len(ramp)
    # Convert to grayscale if needed
    if img.ndim == 3:
        # Use standard luminance formula
        gray = 0.2126 * img[:, :, 0] + 0.7152 * img[:, :, 1] + 0.0722 * img[:, :, 2]
    else:
        gray = img
    # Normalize to 0-1
    norm = (gray - gray.min()) / (gray.ptp() + 1e-6)
    idx = (norm * (n - 1)).astype(int)
    ascii_img = np.array([ramp[i] for i in idx.flat]).reshape(idx.shape)
    return ascii_img


class FrameRateLimiter:
    """
    Frame rate limiter for async frame processing.
    Usage:
        limiter = FrameRateLimiter(target_fps=8)
        while True:
            await limiter.wait()
            ... # process frame
    """

    def __init__(self, target_fps):
        self.target_fps = target_fps
        self.min_interval = 1.0 / target_fps
        self._last_time = None

    async def wait(self):
        now = asyncio.get_event_loop().time()
        if self._last_time is not None:
            elapsed = now - self._last_time
            sleep_time = self.min_interval - elapsed
            if sleep_time > 0:
                await asyncio.sleep(sleep_time)
        self._last_time = asyncio.get_event_loop().time()


class CircularFrameBuffer:
    """
    Circular buffer for video frames with synchronization.
    Usage:
        buf = CircularFrameBuffer(size=8)
        buf.put(frame)
        frame = buf.get()
    """

    def __init__(self, size):
        self.size = size
        self.buffer = [None] * size
        self.start = 0
        self.end = 0
        self.count = 0
        self.lock = threading.Lock()

    def put(self, frame):
        with self.lock:
            self.buffer[self.end] = frame
            self.end = (self.end + 1) % self.size
            if self.count == self.size:
                # Overwrite oldest
                self.start = (self.start + 1) % self.size
            else:
                self.count += 1

    def get(self):
        with self.lock:
            if self.count == 0:
                return None
            frame = self.buffer[self.start]
            self.start = (self.start + 1) % self.size
            self.count -= 1
            return frame

    def clear(self):
        with self.lock:
            self.start = 0
            self.end = 0
            self.count = 0
            self.buffer = [None] * self.size


def process_video_pipeline(img, mode, **kwargs):
    """
    Dispatch video frame to the appropriate processing pipeline.
    mode: 'ascii' or 'sixel'
    kwargs: additional parameters for each pipeline
    """
    if mode == "ascii":
        return process_ascii_pipeline(img, **kwargs)
    elif mode == "sixel":
        return process_sixel_pipeline(img, **kwargs)
    else:
        raise ValueError(f"Unknown pipeline mode: {mode}")


def process_ascii_pipeline(img, **kwargs):
    """
    Process a video frame for ASCII rendering.
    img: RGB numpy array
    kwargs: density, color_mode, etc.
    Returns: ANSI-colored ASCII string for terminal output
    """
    density = kwargs.get("density", get_ascii_density_config())
    color_mode = kwargs.get("color_mode", "256")
    # Get terminal size and calculate frame size
    term_cols, term_rows = get_terminal_size()
    frame_width, frame_height = calculate_ascii_frame_size(term_cols, term_rows)
    # Resize frame
    img_resized = resize_frame(img, frame_width, frame_height)
    # ASCII mapping
    ascii_img = brightness_to_ascii(img_resized, density)
    # Color mapping
    if color_mode == "256":
        color_img = rgb_to_256color(img_resized)
    elif color_mode == "truecolor":
        color_img = img_resized
    else:
        color_img = None
    # Combine ASCII and color
    return ascii_img_to_ansi(ascii_img, color_img, color_mode=color_mode)


def process_sixel_pipeline(img, **kwargs):
    """
    Process a video frame for Sixel rendering.
    img: RGB numpy array
    kwargs: scaling, palette, etc.
    Returns: Sixel-encoded string for terminal output
    """
    # Get terminal size and calculate frame size
    term_cols, term_rows = get_terminal_size()
    # Sixel pixels are square, so use terminal cols/rows directly
    width = kwargs.get("width", term_cols * 6)  # Sixel: 1 col ~ 6px
    height = kwargs.get("height", term_rows * 12)  # Sixel: 1 row ~ 12px
    img_resized = cv2.resize(img, (width, height), interpolation=cv2.INTER_LINEAR)
    if _sixel_available:
        # Encode as Sixel
        buf = sixel.SixelBuffer()
        buf.draw(img_resized)
        return buf.data
    else:
        # Fallback to ASCII
        return process_ascii_pipeline(img, **kwargs)


def ansi_fg_256(idx):
    """
    Return ANSI escape code for 256-color foreground.
    idx: 0-255
    """
    return f"\033[38;5;{idx}m"


def ansi_fg_truecolor(r, g, b):
    """
    Return ANSI escape code for truecolor foreground.
    r, g, b: 0-255
    """
    return f"\033[38;2;{r};{g};{b}m"


def ascii_img_to_ansi(ascii_img, color_img, color_mode="256"):
    """
    Convert a 2D ASCII array and color image to a string with ANSI color codes.
    ascii_img: 2D array of ASCII characters
    color_img: 2D array (H, W) of 256-color indices or (H, W, 3) RGB
    color_mode: '256' or 'truecolor'
    Returns: string for terminal output
    """
    lines = []
    h, w = ascii_img.shape
    for y in range(h):
        line = ""
        for x in range(w):
            ch = ascii_img[y, x]
            if color_mode == "256":
                idx = color_img[y, x]
                line += ansi_fg_256(idx) + ch
            elif color_mode == "truecolor":
                r, g, b = color_img[y, x]
                line += ansi_fg_truecolor(r, g, b) + ch
            else:
                line += ch
        line += "\033[0m"  # Reset at end of line
        lines.append(line)
    return "\n".join(lines)


_ascii_density = "high"


def get_ascii_density_config():
    """
    Get the current ASCII density setting ('high', 'medium', 'low').
    """
    global _ascii_density
    return _ascii_density


def set_ascii_density_config(density):
    """
    Set the ASCII density setting ('high', 'medium', 'low').
    """
    global _ascii_density
    if density in ("high", "medium", "low"):
        _ascii_density = density
    else:
        raise ValueError("Density must be 'high', 'medium', or 'low'")


def get_terminal_size():
    """
    Return (columns, rows) of the terminal window.
    """
    size = shutil.get_terminal_size(fallback=(80, 24))
    return size.columns, size.lines


def calculate_ascii_frame_size(term_cols, term_rows, char_aspect=0.5):
    """
    Calculate optimal frame size for ASCII rendering given terminal size and character aspect ratio.
    char_aspect: width/height ratio of a terminal character (default 0.5 for most fonts)
    Returns: (frame_width, frame_height)
    """
    # Adjust width to account for character aspect ratio
    frame_width = term_cols
    frame_height = int(term_rows / char_aspect)
    return frame_width, frame_height


def optimize_ascii_rendering_pipeline():
    """
    Stub: Optimize ASCII rendering pipeline for minimal latency and efficient string building.
    Implement buffering, adaptive quality, and other optimizations here.
    """
    pass


def monitor_ascii_performance():
    """
    Stub: Monitor ASCII rendering performance and adjust quality adaptively.
    Implement performance tracking and dynamic adjustment here.
    """
    pass


def sixel_supported():
    """
    Stub: Return True if terminal supports Sixel graphics.
    To be implemented with actual detection logic.
    """
    # TODO: Implement Sixel support detection
    return False


def render_sixel(img):
    """
    Stub: Render an RGB numpy array as Sixel graphics.
    To be implemented with libsixel-python.
    """
    # TODO: Implement Sixel rendering
    pass


def render_sixel_scaled(img, width, height):
    """
    Stub: Render an RGB numpy array as Sixel graphics, scaled to (width, height).
    To be implemented with libsixel-python.
    """
    # TODO: Implement Sixel rendering with scaling
    pass


def position_sixel_preview(img, term_cols, term_rows, corner="bottom-right"):
    """
    Stub: Position Sixel preview in the terminal (e.g., bottom-right corner).
    To be implemented with terminal cursor control and Sixel output.
    """
    # TODO: Implement Sixel preview positioning
    pass


def sixel_fallback(img):
    """
    Stub: Fallback to ASCII rendering if Sixel is not supported.
    To be implemented: call ASCII rendering pipeline.
    """
    # TODO: Implement Sixel fallback to ASCII
    pass


class Logger:
    """
    Structured logger with log levels, file and stdout output, and optional user context.
    Usage:
        logger = Logger(user_context)
        logger.info('message')
    """

    LEVELS = {"DEBUG": 10, "INFO": 20, "WARNING": 30, "ERROR": 40}

    def __init__(self, user_context=None, log_file="termcall.log", level="INFO"):
        self.user_context = user_context or {}
        self.log_file = log_file
        self.level = self.LEVELS.get(level.upper(), 20)

    def _format(self, level, msg):
        now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        user = self.user_context.get("email") or self.user_context.get("uid") or "-"
        return f"[{now}] [{level}] [user:{user}] {msg}"

    def _write(self, line):
        print(line, file=sys.stdout)
        with open(self.log_file, "a") as f:
            f.write(line + "\n")

    def log(self, level, msg):
        if self.LEVELS[level] >= self.level:
            line = self._format(level, msg)
            self._write(line)

    def debug(self, msg):
        self.log("DEBUG", msg)

    def info(self, msg):
        self.log("INFO", msg)

    def warning(self, msg):
        self.log("WARNING", msg)

    def error(self, msg):
        self.log("ERROR", msg)


class ConnectionQualityMonitor:
    """
    Monitors and logs connection quality metrics (RTT, packet loss, bitrate).
    Usage:
        monitor = ConnectionQualityMonitor(logger)
        monitor.update(rtt=..., packet_loss=..., bitrate=...)
        print(monitor.indicator())
    """

    def __init__(self, logger=None):
        self.logger = logger
        self.rtt = None
        self.packet_loss = None
        self.bitrate = None

    def update(self, rtt=None, packet_loss=None, bitrate=None):
        if rtt is not None:
            self.rtt = rtt
        if packet_loss is not None:
            self.packet_loss = packet_loss
        if bitrate is not None:
            self.bitrate = bitrate
        if self.logger:
            self.logger.info(
                f"Quality update: RTT={self.rtt}ms, PacketLoss={self.packet_loss}%, Bitrate={self.bitrate}kbps"
            )

    def indicator(self):
        # Simple quality indicator string
        if self.rtt is None or self.packet_loss is None or self.bitrate is None:
            return "[Quality: N/A]"
        score = 100 - (self.packet_loss or 0) - (self.rtt or 0) / 10
        if score > 80:
            return "[Quality: Excellent]"
        elif score > 60:
            return "[Quality: Good]"
        elif score > 40:
            return "[Quality: Fair]"
        else:
            return "[Quality: Poor]"


def list_audio_devices():
    """
    Stub: List available audio input devices.
    Returns a list of device names/IDs.
    """
    # TODO: Integrate with real device enumeration (e.g., sounddevice, pyaudio)
    return ["Default Microphone", "External USB Mic"]


def list_video_devices():
    """
    Stub: List available video input devices.
    Returns a list of device names/IDs.
    """
    # TODO: Integrate with real device enumeration (e.g., OpenCV, ffmpeg)
    return ["Default Camera", "USB Webcam"]


def select_device(devices, prompt="Select device:"):
    """
    CLI prompt to select a device from a list.
    Returns the selected device name/ID.
    """
    print(prompt)
    for i, d in enumerate(devices):
        print(f"  {i+1}. {d}")
    idx = int(input("Enter number: ")) - 1
    return devices[idx]


DEVICE_CONFIG_FILE = os.path.expanduser("~/.termcall_device_config.json")


def load_device_config():
    """
    Load device configuration from JSON file.
    Returns a dict or empty dict if not found.
    """
    if os.path.exists(DEVICE_CONFIG_FILE):
        with open(DEVICE_CONFIG_FILE, "r") as f:
            return json.load(f)
    return {}


def save_device_config(config):
    """
    Save device configuration to JSON file.
    """
    with open(DEVICE_CONFIG_FILE, "w") as f:
        json.dump(config, f, indent=2)
