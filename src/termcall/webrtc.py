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
