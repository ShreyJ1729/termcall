"""
Firebase RTDB WebRTC Signaling Schema (with Firebase Auth UID integration)

/calls/{callId}:
  - caller_uid: str (Firebase Auth UID of caller)
  - callee_uid: str (Firebase Auth UID of callee)
  - state: str (pending, accepted, rejected, timeout)
  - created_at: int (timestamp)
  - updated_at: int (timestamp)

/signaling/{callId}/sdp:
  - type: str (offer or answer)
  - sdp: str (SDP message)
  - sender_uid: str (UID of sender)
  - timestamp: int

/signaling/{callId}/ice:
  - candidate: str (ICE candidate string)
  - sdpMid: str
  - sdpMLineIndex: int
  - sender_uid: str
  - timestamp: int

- All user identification is by Firebase Auth UID, not email.
- Data expiration: signaling/calls should be cleaned up after call ends or timeout (e.g., 30 seconds for unanswered calls).
- Use Firebase security rules to restrict access to signaling data by UID.

Python data models:
"""

from dataclasses import dataclass
from typing import Optional
import time
import uuid
import random

from aiortc import (
    RTCPeerConnection,
    RTCSessionDescription,
    RTCIceCandidate,
    RTCConfiguration,
    RTCIceServer,
)
import asyncio
from aiortc.contrib.media import MediaPlayer
from aiortc.contrib.media import MediaRecorder
from .utils import (
    list_video_devices,
    list_audio_devices,
    select_device,
    load_device_config,
    save_device_config,
)


@dataclass
class Call:
    call_id: str
    caller_uid: str
    callee_uid: str
    state: str  # pending, accepted, rejected, timeout
    created_at: int
    updated_at: int


@dataclass
class SDPMessage:
    type: str  # offer or answer
    sdp: str
    sender_uid: str
    timestamp: int


@dataclass
class ICECandidate:
    candidate: str
    sdpMid: Optional[str]
    sdpMLineIndex: Optional[int]
    sender_uid: str
    timestamp: int


class TermCallPeerConnection:
    def __init__(self, stun_servers=None, turn_servers=None, user_context=None):
        """
        stun_servers: list of STUN server URLs
        turn_servers: list of dicts with keys: urls, username, credential
        user_context: dict with Firebase Auth user/session info
        """
        ice_servers = []
        if stun_servers:
            for url in stun_servers:
                ice_servers.append(RTCIceServer(urls=url))
        if turn_servers:
            for turn in turn_servers:
                ice_servers.append(RTCIceServer(**turn))
        self.pc = RTCPeerConnection(RTCConfiguration(iceServers=ice_servers))
        self.user_context = user_context or {}
        self._frame_callback = None  # User-supplied callback for video frames
        self._frame_tasks = []  # Track running frame receiver tasks
        self.pc.on("connectionstatechange", self._on_connection_state_change)
        self.pc.on("track", self._on_track)

    def _on_connection_state_change(self):
        state = self.pc.connectionState
        user = self.user_context.get("email") or self.user_context.get("uid")
        print(f"[WebRTC] Connection state for {user}: {state}")

    async def create_offer(self):
        offer = await self.pc.createOffer()
        await self.pc.setLocalDescription(offer)
        return self.pc.localDescription

    async def create_answer(self):
        answer = await self.pc.createAnswer()
        await self.pc.setLocalDescription(answer)
        return self.pc.localDescription

    async def set_remote_description(self, sdp, type_):
        desc = RTCSessionDescription(sdp=sdp, type=type_)
        await self.pc.setRemoteDescription(desc)

    async def add_ice_candidate(self, candidate, sdpMid, sdpMLineIndex):
        ice = RTCIceCandidate(
            candidate=candidate,
            sdpMid=sdpMid,
            sdpMLineIndex=sdpMLineIndex,
        )
        await self.pc.addIceCandidate(ice)

    async def add_video_track(self, device=None, width=640, height=480, framerate=30):
        """
        Add a video track from the local camera to the peer connection.
        device: camera device path or None for default
        width, height: resolution (default 480p)
        framerate: target frame rate
        """
        config = load_device_config()
        if not device:
            devices = list_video_devices()
            device = (
                config.get("video_device")
                if config.get("video_device") in devices
                else None
            )
            if not device:
                device = select_device(devices, prompt="Select video device:")
                config["video_device"] = device
                save_device_config(config)
        options = {"framerate": str(framerate), "video_size": f"{width}x{height}"}
        try:
            # On macOS, use avfoundation; on Linux, use v4l2; on Windows, use dshow
            import platform

            sys_platform = platform.system().lower()
            if sys_platform == "darwin":
                player = MediaPlayer(
                    f"avfoundation:{device}", format="avfoundation", options=options
                )
            elif sys_platform == "windows":
                player = MediaPlayer(f"video={device}", format="dshow", options=options)
            else:
                player = MediaPlayer(int(device), format="v4l2", options=options)
        except Exception:
            # Fallback to ffmpeg if v4l2 fails (e.g., on macOS/Windows)
            try:
                player = MediaPlayer(device or None, options=options)
            except Exception as e:
                print(f"[WebRTC] Failed to open video device: {e}")
                return None
        video_track = player.video
        if video_track:
            self.pc.addTrack(video_track)
            print(f"[WebRTC] Added video track at {width}x{height}")
        else:
            print("[WebRTC] No video track available from device.")
        return video_track

    async def add_audio_track(self, device=None, sample_rate=48000):
        """
        Add an audio track from the microphone to the peer connection.
        device: audio device path or None for default
        sample_rate: audio sample rate (default 48000)
        """
        config = load_device_config()
        if not device:
            devices = list_audio_devices()
            device = (
                config.get("audio_device")
                if config.get("audio_device") in devices
                else None
            )
            if not device:
                device = select_device(devices, prompt="Select audio device:")
                config["audio_device"] = device
                save_device_config(config)
        options = {"sample_rate": str(sample_rate)}
        try:
            import platform

            sys_platform = platform.system().lower()
            if sys_platform == "darwin":
                player = MediaPlayer(
                    f"avfoundation:{device}", format="avfoundation", options=options
                )
            elif sys_platform == "windows":
                player = MediaPlayer(f"audio={device}", format="dshow", options=options)
            else:
                player = MediaPlayer(device or None, format=None, options=options)
        except Exception as e:
            print(f"[WebRTC] Failed to open audio device: {e}")
            return None
        audio_track = player.audio
        if audio_track:
            self.pc.addTrack(audio_track)
            print(f"[WebRTC] Added audio track at {sample_rate} Hz")
        else:
            print("[WebRTC] No audio track available from device.")
        return audio_track

    async def setup_audio_output(self, output_device=None, filename=None):
        """
        Optionally setup audio output (playback or recording).
        output_device: device path or None for default
        filename: if set, record to file instead of playback
        """
        if filename:
            recorder = MediaRecorder(filename)
        else:
            recorder = MediaRecorder(output_device or "default")
        return recorder

    async def create_offer_with_constraints(
        self,
        audio=True,
        video=True,
        video_size="640x480",
        audio_codec=None,
        video_codec=None,
    ):
        """
        Create an SDP offer with media constraints and codec preferences.
        """
        # Optionally, set codec preferences here if needed (aiortc uses defaults)
        offer = await self.pc.createOffer()
        await self.pc.setLocalDescription(offer)
        # Optionally, modify SDP for terminal-optimized streaming
        sdp = self.pc.localDescription.sdp
        if video_size:
            # Example: force max resolution in SDP (not always needed)
            sdp = sdp.replace("a=fmtp:", f"a=fmtp:;max-fs={video_size}")
        # Codec filtering can be done here if needed
        return type(self.pc.localDescription)(
            sdp=sdp, type=self.pc.localDescription.type
        )

    async def create_answer_with_constraints(
        self, audio=True, video=True, audio_codec=None, video_codec=None
    ):
        """
        Create an SDP answer with media constraints and codec preferences.
        """
        answer = await self.pc.createAnswer()
        await self.pc.setLocalDescription(answer)
        # Optionally, modify SDP for terminal-optimized streaming
        sdp = self.pc.localDescription.sdp
        # Codec filtering can be done here if needed
        return type(self.pc.localDescription)(
            sdp=sdp, type=self.pc.localDescription.type
        )

    async def process_remote_sdp(self, sdp, type_):
        """
        Set the remote SDP and handle negotiation.
        """
        desc = RTCSessionDescription(sdp=sdp, type=type_)
        await self.pc.setRemoteDescription(desc)

    def close(self):
        return self.pc.close()

    def on_ice_candidate(self, callback):
        """
        Register a callback to be called with each new local ICE candidate.
        """

        @self.pc.on("icecandidate")
        def _on_icecandidate(event):
            if event.candidate:
                callback(event.candidate)

    async def restart_ice(self):
        """
        Restart ICE for connection recovery.
        """
        await self.pc.restartIce()

    def set_connection_state_handler(self, on_state_change):
        """
        Register a callback for connection state changes.
        """
        self._external_state_handler = on_state_change

        @self.pc.on("connectionstatechange")
        def _on_state():
            state = self.pc.connectionState
            if hasattr(self, "_external_state_handler"):
                self._external_state_handler(state)
            if state == "failed":
                asyncio.ensure_future(self._auto_reconnect())

    async def _auto_reconnect(self, max_attempts=5):
        """
        Attempt to reconnect with exponential backoff.
        """
        for attempt in range(1, max_attempts + 1):
            wait = min(2**attempt, 30) + random.uniform(0, 1)
            print(f"[WebRTC] Reconnection attempt {attempt}, waiting {wait:.1f}s...")
            await asyncio.sleep(wait)
            try:
                await self.restart_ice()
                print("[WebRTC] ICE restart triggered.")
                return
            except Exception as e:
                print(f"[WebRTC] ICE restart failed: {e}")
        print("[WebRTC] Max reconnection attempts reached. Giving up.")

    def monitor_connection_quality(self):
        """
        Stub for connection quality monitoring (to be implemented).
        """
        print("[WebRTC] Connection quality monitoring not yet implemented.")

    async def terminate_call(self):
        """
        Gracefully terminate the call, close all tracks and connections, and cleanup resources.
        """
        print("[WebRTC] Terminating call and cleaning up resources...")
        # Stop all tracks
        for sender in self.pc.getSenders():
            track = sender.track
            if track:
                await track.stop()
        # Close peer connection
        await self.pc.close()
        print("[WebRTC] Peer connection closed.")
        # Additional cleanup for Firebase Auth session can be added here if needed
        # (e.g., sign out, clear tokens, etc.)

    @staticmethod
    def filter_and_validate_candidate(candidate):
        """
        Filter and validate an ICE candidate (basic checks).
        """
        if not candidate or not candidate.candidate:
            return False
        # Example: filter out host candidates if needed
        # if "typ host" in candidate.candidate:
        #     return False
        return True

    def on_video_frame(self, callback):
        """
        Register a callback to be called with each received video frame.
        Callback signature: async def callback(frame, track):
        """
        self._frame_callback = callback

    def _on_track(self, track):
        print(f"[WebRTC] Track received: kind={track.kind}")
        if track.kind == "video":
            # Start a background task to receive frames
            task = asyncio.ensure_future(self._recv_video_frames(track))
            self._frame_tasks.append(task)

    async def _recv_video_frames(self, track):
        print(f"[WebRTC] Starting frame reception for track: {track}")
        try:
            while True:
                frame = await track.recv()
                # Optionally, detect frame format here (I420, RGB, etc.)
                if self._frame_callback:
                    await self._frame_callback(frame, track)
        except Exception as e:
            print(f"[WebRTC] Frame reception error: {e}")
        finally:
            print(f"[WebRTC] Frame reception ended for track: {track}")


def create_call_request(caller_uid, callee_uid, id_token):
    """Create a call request in RTDB with state 'pending'. Returns callId."""
    from .firebase import get_firebase

    _, _, db = get_firebase()
    call_id = str(uuid.uuid4())
    now = int(time.time())
    call_data = {
        "caller_uid": caller_uid,
        "callee_uid": callee_uid,
        "state": "pending",
        "created_at": now,
        "updated_at": now,
    }
    db.child("calls").child(call_id).set(call_data, id_token)
    return call_id


def update_call_state(call_id, new_state, id_token):
    """Update the state of a call (pending/accepted/rejected/timeout)."""
    from .firebase import get_firebase

    _, _, db = get_firebase()
    now = int(time.time())
    db.child("calls").child(call_id).update(
        {"state": new_state, "updated_at": now}, id_token
    )


def get_call(call_id, id_token):
    """Fetch call data from RTDB."""
    from .firebase import get_firebase

    _, _, db = get_firebase()
    return db.child("calls").child(call_id).get(id_token).val()


def send_sdp(call_id, sdp_type, sdp, sender_uid, id_token):
    """Send SDP offer or answer to /signaling/{callId}/sdp."""
    from .firebase import get_firebase

    _, _, db = get_firebase()
    now = int(time.time())
    sdp_data = {
        "type": sdp_type,  # 'offer' or 'answer'
        "sdp": sdp,
        "sender_uid": sender_uid,
        "timestamp": now,
    }
    db.child("signaling").child(call_id).child("sdp").set(sdp_data, id_token)


def get_sdp(call_id, id_token):
    """Fetch SDP offer/answer from /signaling/{callId}/sdp."""
    from .firebase import get_firebase

    _, _, db = get_firebase()
    return db.child("signaling").child(call_id).child("sdp").get(id_token).val()


def send_ice_candidate(call_id, candidate, sdpMid, sdpMLineIndex, sender_uid, id_token):
    """Send an ICE candidate to /signaling/{callId}/ice (append, not overwrite)."""
    from .firebase import get_firebase

    _, _, db = get_firebase()
    now = int(time.time())
    ice_data = {
        "candidate": candidate,
        "sdpMid": sdpMid,
        "sdpMLineIndex": sdpMLineIndex,
        "sender_uid": sender_uid,
        "timestamp": now,
    }
    # Push as a new child under /signaling/{callId}/ice
    db.child("signaling").child(call_id).child("ice").push(ice_data, id_token)


def get_ice_candidates(call_id, id_token):
    """Fetch all ICE candidates for a call from /signaling/{callId}/ice. Returns a list of ICECandidate objects."""
    from .firebase import get_firebase

    _, _, db = get_firebase()
    ice_snap = db.child("signaling").child(call_id).child("ice").get(id_token)
    candidates = []
    if ice_snap.each():
        for item in ice_snap.each():
            val = item.val()
            candidates.append(
                ICECandidate(
                    candidate=val.get("candidate"),
                    sdpMid=val.get("sdpMid"),
                    sdpMLineIndex=val.get("sdpMLineIndex"),
                    sender_uid=val.get("sender_uid"),
                    timestamp=val.get("timestamp"),
                )
            )
    return candidates


def listen_for_incoming_calls(local_uid, id_token, callback):
    """
    Listen for incoming call requests targeting the given local_uid.
    Calls the callback(event) on new/updated call requests where callee_uid == local_uid.
    """
    from .firebase import get_firebase

    _, _, db = get_firebase()
    # Listen to all calls, filter in callback
    stream = db.child("calls").stream(callback, id_token)
    return stream  # Caller should keep reference to stop the stream later


def listen_for_signaling_updates(call_id, id_token, callback):
    """
    Listen for signaling updates (SDP, ICE) for a given call_id.
    Calls the callback(event) on any change under /signaling/{callId}.
    """
    from .firebase import get_firebase

    _, _, db = get_firebase()
    stream = db.child("signaling").child(call_id).stream(callback, id_token)
    return stream


def check_and_timeout_pending_calls(id_token, timeout_seconds=30):
    """
    Check for 'pending' calls older than timeout_seconds and set their state to 'timeout'.
    Should be run periodically or after call attempts.
    """
    from .firebase import get_firebase
    import time

    _, _, db = get_firebase()
    now = int(time.time())
    calls = db.child("calls").get(id_token)
    if not calls.each():
        return []
    timed_out = []
    for item in calls.each():
        call = item.val()
        call_id = item.key()
        if (
            call.get("state") == "pending"
            and now - call.get("created_at", now) > timeout_seconds
        ):
            db.child("calls").child(call_id).update(
                {"state": "timeout", "updated_at": now}, id_token
            )
            timed_out.append(call_id)
    return timed_out


def cleanup_signaling_data(call_id, id_token):
    """
    Delete signaling and call data for a given call_id (after timeout, rejection, or call end).
    """
    from .firebase import get_firebase

    _, _, db = get_firebase()
    db.child("signaling").child(call_id).remove(id_token)
    db.child("calls").child(call_id).remove(id_token)


# Example callback usage:
# def on_call_event(event):
#     print(f"Call event: {event['event']} {event['path']} {event['data']}")
# def on_signaling_event(event):
#     print(f"Signaling event: {event['event']} {event['path']} {event['data']}")
#
# call_stream = listen_for_incoming_calls(local_uid, id_token, on_call_event)
# signaling_stream = listen_for_signaling_updates(call_id, id_token, on_signaling_event)
# ...
# call_stream.close()  # To stop listening
# signaling_stream.close()
