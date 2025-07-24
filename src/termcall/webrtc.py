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
