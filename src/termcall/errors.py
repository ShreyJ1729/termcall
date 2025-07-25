class CallError(Exception):
    """Base exception for call flow errors."""

    pass


class AuthError(CallError):
    """Exception for authentication-related errors."""

    pass


class NetworkError(CallError):
    """Exception for network-related errors."""

    pass


class DeviceError(CallError):
    """Exception for device-related errors (audio/video)."""

    pass


class WebRTCError(CallError):
    """Exception for WebRTC signaling/connection errors."""

    pass


def handle_error(error):
    """
    Centralized error handler. Prints user-friendly messages and logs errors (stub).
    """
    # TODO: Add logging, severity levels, and retry logic
    if isinstance(error, AuthError):
        print(f"[AUTH ERROR] {error}")
    elif isinstance(error, NetworkError):
        print(f"[NETWORK ERROR] {error}")
    elif isinstance(error, DeviceError):
        print(f"[DEVICE ERROR] {error}")
    elif isinstance(error, WebRTCError):
        print(f"[WEBRTC ERROR] {error}")
    elif isinstance(error, CallError):
        print(f"[CALL ERROR] {error}")
    else:
        print(f"[ERROR] {error}")
