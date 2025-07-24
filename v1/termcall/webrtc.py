import asyncio
import numpy as np
from aiortc import (
    RTCPeerConnection,
    RTCSessionDescription,
    RTCIceCandidate,
    VideoStreamTrack,
    RTCConfiguration,
    RTCIceServer,
)
from aiortc.contrib.media import MediaPlayer
from .firebase import signaling_ref, call_requests_ref, users_ref, clear_screen
from .rendering import move_cursor, write_sixel
import sys
import threading
from pynput import keyboard
from image_to_ascii import ImageToAscii

from prompt_toolkit.completion import WordCompleter
from prompt_toolkit.shortcuts import CompleteStyle


# --- Restore CameraVideoTrack ---
class CameraVideoTrack(VideoStreamTrack):
    """
    A VideoStreamTrack that captures video from the local webcam using OpenCV.
    Releases the camera on close.
    """

    def __init__(self):
        super().__init__()
        import cv2

        self.cap = cv2.VideoCapture(0)
        self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
        self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
        self._closed = False
        self._lock = threading.Lock()

    async def recv(self):
        pts, time_base = await self.next_timestamp()
        import cv2
        import numpy as np

        with self._lock:
            if self._closed or not self.cap.isOpened():
                frame = np.zeros((480, 640, 3), np.uint8)
            else:
                ret, frame = self.cap.read()
                if not ret:
                    frame = np.zeros((480, 640, 3), np.uint8)
        frame = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
        from av import VideoFrame

        video_frame = VideoFrame.from_ndarray(frame, format="rgb24")
        video_frame.pts = pts
        video_frame.time_base = time_base
        return video_frame

    def close(self):
        with self._lock:
            self._closed = True
            if self.cap.isOpened():
                self.cap.release()


# --- Restore webrtc_signaling_flow ---
async def webrtc_signaling_flow(
    is_caller, my_email, peer_email, call_id, loopback=False
):
    import datetime
    import psutil
    from aiortc import RTCConfiguration, RTCIceServer

    def log(msg):
        print(
            f"[DEBUG][{datetime.datetime.now().strftime('%H:%M:%S.%f')[:-3]}][{'CALLER' if is_caller else 'CALLEE'}] {msg}"
        )

    # Print all available network interfaces for debugging
    log("Available network interfaces:")
    try:
        for name, addrs in psutil.net_if_addrs().items():
            for addr in addrs:
                log(f"  {name}: {addr.address} ({addr.family})")
    except Exception as e:
        log(f"Could not list network interfaces: {e}")
    # Use both STUN and host candidates
    # aiortc default iceTransportPolicy is 'all', which is permissive
    # --- Use disambiguated signaling keys for loopback ---
    if loopback and my_email == peer_email:
        my_key = my_email.replace(".", ",") + ("__caller" if is_caller else "__callee")
        peer_key = my_email.replace(".", ",") + (
            "__callee" if is_caller else "__caller"
        )
    else:
        my_key = my_email.replace(".", ",")
        peer_key = peer_email.replace(".", ",")
    pc = RTCPeerConnection(
        RTCConfiguration(
            iceServers=[
                RTCIceServer(urls=["stun:stun.l.google.com:19302"]),
                RTCIceServer(urls=[]),  # Enables host (local) candidates
            ]
        )
    )
    gathered_ice = []
    my_sig_ref = signaling_ref.child(call_id).child(my_key)
    peer_sig_ref = signaling_ref.child(call_id).child(peer_key)

    @pc.on("icecandidate")
    async def on_icecandidate(event):
        log(f"on_icecandidate called. event.candidate: {event.candidate}")
        if event.candidate:
            gathered_ice.append(event.candidate)
            candidates = [c.to_sdp() for c in gathered_ice]
            log(f"Setting ICE candidates: {len(candidates)} candidates: {candidates}")
            my_sig_ref.update({"candidates": candidates})
            await asyncio.sleep(0)
        else:
            log("ICE gathering complete (event.candidate is None)")

    @pc.on("iceconnectionstatechange")
    async def on_iceconnectionstatechange():
        log(f"ICE connection state changed: {pc.iceConnectionState}")

    if is_caller:
        log("Creating offer...")
        offer = await pc.createOffer()
        await pc.setLocalDescription(offer)
        log("Setting offer in signaling...")
        my_sig_ref.set(
            {
                "offer": offer.sdp,
                "type": offer.type,
                "candidates": [],
            }
        )
        await asyncio.sleep(0)
        log("Waiting for answer from callee...")
        while True:
            data = peer_sig_ref.get() or {}
            if data.get("answer"):
                log("Received answer from callee.")
                answer = RTCSessionDescription(sdp=data["answer"], type="answer")
                await pc.setRemoteDescription(answer)
                break
            await asyncio.sleep(1)
    else:
        log("Waiting for offer from caller...")
        while True:
            data = peer_sig_ref.get() or {}
            if data.get("offer"):
                log("Received offer from caller.")
                offer = RTCSessionDescription(sdp=data["offer"], type="offer")
                await pc.setRemoteDescription(offer)
                break
            await asyncio.sleep(1)
        log("Creating answer...")
        answer = await pc.createAnswer()
        await pc.setLocalDescription(answer)
        log("Setting answer in signaling...")
        my_sig_ref.set(
            {
                "answer": answer.sdp,
                "type": answer.type,
                "candidates": [],
            }
        )
        await asyncio.sleep(0)

    log("Exchanging ICE candidates...")
    seen_candidates = set()
    while True:
        data = peer_sig_ref.get() or {}
        candidates = data.get("candidates", [])
        log(f"ICE candidates received from peer: {len(candidates)}: {candidates}")
        for c_sdp in candidates:
            if c_sdp not in seen_candidates:
                seen_candidates.add(c_sdp)
                log(f"Adding ICE candidate from peer: {c_sdp}")
                candidate = RTCIceCandidate.from_sdp(c_sdp)
                await pc.addIceCandidate(candidate)
        if pc.iceConnectionState in ("connected", "completed"):
            log(
                f"ICE connection state: {pc.iceConnectionState}. Connection established."
            )
            break
        await asyncio.sleep(1)
    log("WebRTC signaling complete. Peer connection established.")
    return pc


# ... other functions ...


async def initiate_call_request(caller_email, callee_email):
    import time

    request = {
        "caller_email": caller_email,
        "callee_email": callee_email,
        "status": "pending",
        "timestamp": int(time.time()),
    }
    new_ref = call_requests_ref.push(request)
    request_id = new_ref.key
    print(f"Call request sent to {callee_email}. Waiting for response...")
    while True:
        result = call_requests_ref.child(request_id).get()
        if not result:
            print("Call request was removed or not found.")
            return None
        status = result.get("status")
        if status == "pending":
            await asyncio.sleep(1)
            continue
        elif status == "accepted":
            print(f"Call accepted by {callee_email}!")
            # Start signaling as caller
            await handle_call_flow(True, caller_email, callee_email, request_id)
            return True
        elif status == "declined":
            print(f"Call declined by {callee_email}.")
            return False
        else:
            print(f"Unknown call request status: {status}")
            return None


async def listen_for_incoming_calls(my_email):
    from prompt_toolkit import PromptSession
    import time

    session = PromptSession()
    print("Listening for incoming calls...")
    while True:
        all_requests = call_requests_ref.get() or {}
        for req_id, req in all_requests.items():
            if req.get("callee_email") == my_email and req.get("status") == "pending":
                caller = req.get("caller_email")
                clear_screen()
                print(f"\nIncoming call from {caller}!")
                while True:
                    response = await session.prompt_async(
                        f"Accept call from {caller}? [y/n]: "
                    )
                    if response.lower() in ["y", "n"]:
                        break
                new_status = "accepted" if response.lower() == "y" else "declined"
                call_requests_ref.child(req_id).update({"status": new_status})
                if new_status == "accepted":
                    print("Call accepted. Proceeding to call setup...")
                    # Start signaling as callee
                    await handle_call_flow(False, my_email, caller, req_id)
                else:
                    print("Call declined.")
        await asyncio.sleep(2)


async def handle_call_flow(is_caller, my_email, peer_email, call_id, loopback=False):
    """
    Handles the full call flow after call is accepted (signaling + media streaming + video rendering + audio playback + controls + cleanup + perf).
    """
    try:
        pc = await webrtc_signaling_flow(
            is_caller, my_email, peer_email, call_id, loopback=loopback
        )
    except Exception as e:
        print(f"[Error] Failed to establish connection: {e}")
        return

    # --- Add local video and audio tracks ---
    print("Adding local video and audio tracks...")
    video_track = CameraVideoTrack()
    try:
        pc.addTrack(video_track)
    except Exception as e:
        print(f"[Error] Could not add video track: {e}")
    # Use aiortc MediaPlayer for audio (microphone)
    try:
        player = MediaPlayer("default", format="pulse")  # Linux PulseAudio
    except Exception:
        try:
            player = MediaPlayer(None)  # Try default device
        except Exception:
            player = None
    if player and player.audio:
        try:
            pc.addTrack(player.audio)
        except Exception as e:
            print(f"[Error] Could not add audio track: {e}")
    else:
        print("Warning: No audio input device found.")

    # --- Video rendering setup ---
    remote_video_queue = asyncio.Queue(maxsize=2)
    local_video_queue = asyncio.Queue(maxsize=2)
    remote_audio_tracks = []
    mute_audio = False
    mute_video = False
    call_active = True
    last_ascii = None
    last_local = None

    # --- Handle remote tracks ---
    @pc.on("track")
    def on_track(track):
        if track.kind == "video":
            print("[Remote video track received] (Rendering as ASCII)")

            async def recv_remote():
                while call_active:
                    try:
                        frame = await track.recv()
                        img = frame.to_ndarray(format="rgb24")
                        await remote_video_queue.put(img)
                    except Exception:
                        break

            asyncio.create_task(recv_remote())
        elif track.kind == "audio":
            print("[Remote audio track received] (Playing back)")
            remote_audio_tracks.append(track)

            async def play_audio():
                from aiortc.contrib.media import MediaPlayer as Player

                try:
                    recorder = MediaPlayer(None)
                except Exception:
                    recorder = None
                if recorder:
                    while call_active:
                        try:
                            frame = await track.recv()
                            if not mute_audio:
                                await recorder.audio._track._queue.put(frame)
                        except Exception:
                            break
                else:
                    while call_active:
                        try:
                            await track.recv()
                        except Exception:
                            break

            asyncio.create_task(play_audio())

    # --- Local video preview (Sixel) ---
    async def local_preview():
        while call_active:
            try:
                frame = await video_track.recv()
                img = frame.to_ndarray(format="rgb24")
                await local_video_queue.put(img)
            except Exception:
                break

    asyncio.create_task(local_preview())

    # --- Rendering loop ---
    async def render_loop():
        nonlocal last_ascii, last_local
        clear_screen()
        print("\n[Controls] m: mute audio, v: mute video, q: quit\n")
        while call_active:
            # Render remote video as ASCII art
            if not remote_video_queue.empty():
                img = await remote_video_queue.get()
                if not mute_video:
                    ascii_converter = ImageToAscii(
                        img, width=106, height=60, colored=True
                    )
                    ascii_art = ascii_converter.convert()
                    if ascii_art != last_ascii:
                        move_cursor(3, 1)
                        print(ascii_art, end="")
                        last_ascii = ascii_art
            # Render local video as Sixel in bottom right
            if not local_video_queue.empty():
                img = await local_video_queue.get()
                if not mute_video:
                    if not np.array_equal(img, last_local):
                        move_cursor(62, 120)
                        write_sixel(img, sys.stdout, width=100, height=100)
                        last_local = img.copy()
            await asyncio.sleep(0.07)  # ~14 FPS

    render_task = asyncio.create_task(render_loop())

    # --- Keyboard controls ---
    def on_press(key):
        nonlocal mute_audio, mute_video, call_active
        try:
            if key.char == "m":
                mute_audio = not mute_audio
                print(f"\n[Audio {'muted' if mute_audio else 'unmuted'}]")
            elif key.char == "v":
                mute_video = not mute_video
                print(f"\n[Video {'muted' if mute_video else 'unmuted'}]")
            elif key.char == "q":
                print("\n[Quitting call]")
                call_active = False
                return False  # Stop listener
        except Exception:
            pass

    listener = keyboard.Listener(on_press=on_press)
    listener.start()

    print(
        "Media streaming, video rendering, and audio playback started. Press 'm' to mute audio, 'v' to mute video, 'q' to quit."
    )
    try:
        while call_active:
            await asyncio.sleep(0.2)
    except (KeyboardInterrupt, asyncio.CancelledError):
        print("Call ended.")
    finally:
        # --- Cleanup resources ---
        print("\nCleaning up call resources...")
        render_task.cancel()
        listener.stop()
        try:
            video_track.close()
        except Exception:
            pass
        try:
            if player:
                player.audio and player.audio.stop()
                player and player.stop()
        except Exception:
            pass
        try:
            await pc.close()
        except Exception:
            pass
        # try:
        #     signaling_ref.child(call_id).delete()
        # except Exception:
        #     pass
        clear_screen()
        print("\nCall ended. Returning to main menu.\n")


async def browse_users_for_call(current_email):
    users = users_ref.get() or {}
    if not users:
        clear_screen()
        print("No other users found.")
        return None
    user_list = [
        (e.replace(",", "."), d.get("full_name", e))
        for e, d in users.items()
        if e.replace(",", ".") != current_email
    ]
    if not user_list:
        clear_screen()
        print("No other users to call.")
        return None
    clear_screen()
    from prompt_toolkit import PromptSession
    from prompt_toolkit.completion import WordCompleter
    from prompt_toolkit.shortcuts import CompleteStyle

    completer = WordCompleter(
        [f"{name} <{email}>" for email, name in user_list], ignore_case=True
    )
    print("\nRegistered Users:")
    for email, name in user_list:
        print(f"  {name} ({email})")
    session = PromptSession()
    while True:
        selected = await session.prompt_async(
            "Type or select a user to call (Tab for options, Enter to select): ",
            completer=completer,
            complete_style=CompleteStyle.COLUMN,
        )
        import re

        match = re.search(r"<([^>]+)>", selected)
        if match:
            sel_email = match.group(1)
            for email, name in user_list:
                if email == sel_email:
                    print(f"Selected: {name} ({email})")
                    return email, name
            for email, name in user_list:
                if selected.strip() == email or selected.strip() == name:
                    print(f"Selected: {name} ({email})")
                    return email, name
        print("Invalid selection. Try again.")
