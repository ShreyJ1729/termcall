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
