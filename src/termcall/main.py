import click
import os
import secrets
from .auth import (
    validate_session,
    save_session,
    load_session,
    register_user,
    login_user,
)

TERMCALL_DIR = os.path.expanduser("~/.termcall")


@click.group()
@click.version_option("0.1.0", prog_name="TermCall")
def main():
    """TermCall CLI - Terminal-based video calls with Firebase and WebRTC."""
    pass


@main.command()
@click.argument("email")
def login(email):
    """Login or register to your TermCall account (auto password)."""
    from .ui import show_status, show_error

    if not os.path.exists(TERMCALL_DIR):
        os.makedirs(TERMCALL_DIR)
    pw_file = os.path.join(TERMCALL_DIR, f"{email}.pw")
    # Check for existing session
    valid, session = validate_session()
    if valid:
        show_status(f"Already logged in as {email}.")
        return
    # Try to load password from file
    if os.path.exists(pw_file):
        with open(pw_file, "r") as f:
            password = f.read().strip()
        user = login_user(email, password)
        if user and isinstance(user, dict):
            save_session(
                user["idToken"], user["refreshToken"], user["localId"], email=email
            )
            show_status(f"Logged in as {email}.")
            return
        else:
            show_error("Login failed. Try deleting your .pw file and re-running.")
            return
    # Registration workflow
    click.echo("No account found. Registering new user...")
    full_name = click.prompt("Enter your full name")
    password = secrets.token_urlsafe(16)
    result = register_user(email, password, full_name)
    if "successfully" in result:
        with open(pw_file, "w") as f:
            f.write(password)
        user = login_user(email, password)
        if user and isinstance(user, dict):
            save_session(
                user["idToken"],
                user["refreshToken"],
                user["localId"],
                email=email,
                full_name=full_name,
            )
            show_status(f"Registered and logged in as {email}.")
        else:
            show_error("Registration succeeded but login failed. Try again.")
    else:
        show_error(result)


@main.command()
def logout():
    """Logout and clear session."""
    click.echo("Logging out (stub)")


@main.command()
def list():
    """List all users (from Firebase profile directory, with cache)."""
    from .auth import load_session, get_user_schema
    from .firebase import get_firebase
    from .utils import get_profiles_offline_first
    from .ui import show_status, show_error

    session = load_session()
    if not session:
        show_error("Not logged in.")
        return
    id_token = session["idToken"]
    local_id = session["localId"]
    email = session.get("email")
    full_name = session.get("full_name", "")
    # Ensure current user's profile is present in RTDB
    _, _, db = get_firebase()
    user_profile = db.child("users").child(local_id).get(id_token).val()
    if not user_profile:
        user_data = get_user_schema(email, full_name, "")
        db.child("users").child(local_id).set(user_data, id_token)
    try:
        profiles = get_profiles_offline_first(
            id_token, "user_profiles", 300
        )  # 5 min cache
    except Exception as e:
        show_error(f"Failed to load user profiles: {e}")
        return
    if not profiles:
        show_status("No users found.")
        return
    for p in profiles:
        marker = "*" if p.get("email") and p.get("email") == email else " "
        print(f"{marker} {p.get('email', ''):30} {p.get('full_name', '')}")
    print("\n* = you")


@main.command()
@click.argument("query", required=False)
def search(query):
    """Search for users by email or name."""
    from .auth import load_session
    from .utils import get_profiles_offline_first, filter_user_profiles
    from .ui import show_status, show_error

    session = load_session()
    if not session:
        show_error("Not logged in.")
        return
    id_token = session["idToken"]
    try:
        profiles = get_profiles_offline_first(id_token, "user_profiles", 300)
    except Exception as e:
        show_error(f"Failed to load user profiles: {e}")
        return
    if not profiles:
        show_status("No users found.")
        return
    results = filter_user_profiles(profiles, query)
    if not results:
        show_status(f"No users found matching '{query}'.")
        return
    print(f"Found {len(results)} user(s) matching '{query}':\n")
    for p in results:
        print(f"{p.get('email','') :30} {p.get('full_name','')}")


@main.command()
def end():
    """End the current call."""
    click.echo("Ending call (stub)")


def initiate_call_signaling(caller_email, callee_email):
    # REMOVE THIS STUB
    pass


def handle_connection_and_rendering(call_id, caller_email, callee_email):
    # REMOVE THIS STUB
    pass


@main.command()
@click.argument("email")
def videocall(email):
    """
    Orchestrate the complete call flow: authentication, call initiation, connection, rendering, and cleanup.
    Usage: termcall videocall <callee_email>
    """
    import asyncio
    from .auth import validate_session
    from .ui import show_status, show_error
    from .utils import Logger, get_profiles_offline_first, sixel_supported
    from .errors import handle_error, NetworkError, DeviceError, WebRTCError
    from .webrtc import (
        create_call_request,
        send_sdp,
        get_sdp,
        send_ice_candidate,
        get_ice_candidates,
        TermCallPeerConnection,
        cleanup_signaling_data,
    )

    logger = Logger()
    try:
        show_status("Checking authentication...")
        valid, session = validate_session()
        if not valid or not session:
            raise NetworkError(
                "Not authenticated. Please run 'termcall login <email>' first."
            )
        caller_email = session.get("email", "[unknown]")
        caller_uid = session.get("localId") or session.get("userId")
        id_token = session.get("idToken")
        if not caller_uid or not id_token:
            raise NetworkError("Session missing UID or idToken.")
        logger.info(f"Authenticated as {caller_email} (uid={caller_uid})")
        # Map callee email to UID
        show_status(f"Looking up callee UID for {email}...")
        profiles = get_profiles_offline_first(id_token, "user_profiles", 300)
        callee_profile = next((p for p in profiles if p.get("email") == email), None)
        if not callee_profile:
            raise NetworkError(f"No user found with email: {email}")
        callee_uid = callee_profile.get("uid") or callee_profile.get("localId")
        if not callee_uid:
            # Try to infer UID from RTDB key if present
            callee_uid = callee_profile.get("id")
        if not callee_uid:
            raise NetworkError(f"Could not determine UID for {email}")
        logger.info(f"Callee UID: {callee_uid}")
        # Initiate call signaling
        show_status(f"Initiating call to {email}...")
        call_id = create_call_request(caller_uid, callee_uid, id_token)
        logger.info(f"Call signaling initiated, call_id={call_id}")

        # Setup WebRTC peer connection
        async def call_flow():
            pc = TermCallPeerConnection(
                user_context={"email": caller_email, "uid": caller_uid}
            )
            await pc.add_video_track()
            await pc.add_audio_track()
            # Create offer
            offer = await pc.create_offer()
            send_sdp(call_id, "offer", offer.sdp, caller_uid, id_token)
            logger.info("SDP offer sent.")
            # Wait for answer
            show_status("Waiting for callee to answer...")
            answer = None
            for _ in range(30):  # Wait up to ~30 seconds
                sdp = get_sdp(call_id, id_token)
                if sdp and sdp.get("type") == "answer":
                    answer = sdp
                    break
                await asyncio.sleep(1)
            if not answer:
                raise WebRTCError("Call timed out waiting for answer.")
            await pc.set_remote_description(answer["sdp"], answer["type"])
            logger.info("SDP answer received and set.")
            # ICE candidate exchange (basic polling)
            show_status("Exchanging ICE candidates...")
            # Send local ICE candidates (not implemented in this stub, but aiortc can do this)
            # Receive remote ICE candidates
            for _ in range(10):
                candidates = get_ice_candidates(call_id, id_token)
                for c in candidates:
                    if c.sender_uid != caller_uid:
                        await pc.add_ice_candidate(
                            c.candidate, c.sdpMid, c.sdpMLineIndex
                        )
                await asyncio.sleep(1)
            # Video frame callback (real rendering)
            from .utils import (
                process_ascii_pipeline,
                process_sixel_pipeline,
                FrameRateLimiter,
            )

            mode = "sixel" if sixel_supported() else "ascii"
            limiter = FrameRateLimiter(target_fps=8)
            import sys

            def on_frame(frame, track):
                img = frame.to_ndarray(format="rgb24")
                if mode == "sixel":
                    rendered = process_sixel_pipeline(img)
                else:
                    rendered = process_ascii_pipeline(img)
                print("\033[H", end="")  # Move cursor to top left
                print(rendered, end="\r", flush=True)

            pc.on_video_frame(on_frame)
            # Wait for call to end (stub: 30 seconds)
            show_status("Call established! (stub: will end in 30s)")
            await asyncio.sleep(30)
            await pc.terminate_call()
            cleanup_signaling_data(call_id, id_token)
            show_status("Call ended and cleaned up.")

        asyncio.run(call_flow())
    except Exception as e:
        handle_error(e)
        logger.error(f"Exception in videocall: {e}")
        show_error("A problem occurred during the call. See logs for details.")


@main.command()
def listen():
    """
    Run a persistent listener that automatically answers incoming calls for the authenticated user.
    Usage: termcall listen
    """
    import asyncio
    from .auth import validate_session
    from .ui import show_status, show_error
    from .utils import Logger, sixel_supported
    from .errors import handle_error, NetworkError, DeviceError, WebRTCError
    from .webrtc import (
        listen_for_incoming_calls,
        get_call,
        get_sdp,
        send_sdp,
        get_ice_candidates,
        send_ice_candidate,
        TermCallPeerConnection,
        cleanup_signaling_data,
    )
    from time import time

    logger = Logger()
    try:
        show_status("Checking authentication...")
        valid, session = validate_session()
        if not valid or not session:
            raise NetworkError(
                "Not authenticated. Please run 'termcall login <email>' first."
            )
        user_email = session.get("email") or "[unknown]"
        user_uid = session.get("localId") or session.get("userId")
        id_token = session.get("idToken")
        if not user_uid or not id_token:
            raise NetworkError("Session missing UID or idToken.")
        logger.info(f"Authenticated as {user_email} (uid={user_uid})")
        show_status(f"Listening for incoming calls as {user_email}...")

        async def answer_call(call_id, caller_uid):
            logger.info(f"Incoming call from {caller_uid}, call_id={call_id}")
            show_status(f"Answering call from {caller_uid}...")
            pc = TermCallPeerConnection(
                user_context={"email": user_email, "uid": user_uid}
            )
            await pc.add_video_track()
            await pc.add_audio_track()
            # Wait for offer
            offer = None
            for _ in range(30):
                sdp = get_sdp(call_id, id_token)
                if sdp and sdp.get("type") == "offer":
                    offer = sdp
                    break
                await asyncio.sleep(1)
            if not offer:
                logger.error("Timed out waiting for SDP offer.")
                return
            await pc.set_remote_description(offer["sdp"], offer["type"])
            logger.info("SDP offer received and set.")
            # Create and send answer
            answer = await pc.create_answer()
            send_sdp(call_id, "answer", answer.sdp, user_uid, id_token)
            logger.info("SDP answer sent.")
            # ICE candidate exchange (basic polling)
            show_status("Exchanging ICE candidates...")
            for _ in range(10):
                candidates = get_ice_candidates(call_id, id_token)
                for c in candidates:
                    if c.sender_uid != user_uid:
                        await pc.add_ice_candidate(
                            c.candidate, c.sdpMid, c.sdpMLineIndex
                        )
                await asyncio.sleep(1)
            # Video frame callback (real rendering)
            from .utils import (
                process_ascii_pipeline,
                process_sixel_pipeline,
                FrameRateLimiter,
            )

            mode = "sixel" if sixel_supported() else "ascii"
            limiter = FrameRateLimiter(target_fps=8)
            import sys

            def on_frame(frame, track):
                img = frame.to_ndarray(format="rgb24")
                if mode == "sixel":
                    rendered = process_sixel_pipeline(img)
                else:
                    rendered = process_ascii_pipeline(img)
                print("\033[H", end="")  # Move cursor to top left
                print(rendered, end="\r", flush=True)

            pc.on_video_frame(on_frame)
            show_status("Call established! (stub: will end in 30s)")
            await asyncio.sleep(30)
            await pc.terminate_call()
            cleanup_signaling_data(call_id, id_token)
            show_status("Call ended and cleaned up.")

        def run_listener():
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            main_loop = loop

            def on_call_event(event):
                # event['data'] is the call object, event['path'] is the RTDB path
                if event["event"] not in ("put", "patch"):
                    return
                data = event["data"]
                if not data:
                    return
                # If this is a new call or update, check if we're the callee and state is pending
                if isinstance(data, dict):
                    # If this is a full call object
                    call = data
                    call_id = event["path"].strip("/")
                    if (
                        call.get("callee_uid") == user_uid
                        and call.get("state") == "pending"
                    ):
                        asyncio.run_coroutine_threadsafe(
                            answer_call(call_id, call.get("caller_uid")), main_loop
                        )
                # If this is a patch, event['path'] may be /<call_id>/field
                elif event["path"] and event["path"].count("/") == 2:
                    # /<call_id>/field
                    call_id = event["path"].split("/")[1]
                    call = get_call(call_id, id_token)
                    if (
                        call
                        and call.get("callee_uid") == user_uid
                        and call.get("state") == "pending"
                    ):
                        asyncio.run_coroutine_threadsafe(
                            answer_call(call_id, call.get("caller_uid")), main_loop
                        )

            stream = listen_for_incoming_calls(user_uid, id_token, on_call_event)
            try:
                main_loop.run_forever()
            except KeyboardInterrupt:
                show_status("Listener stopped by user.")
                stream.close()

        run_listener()
    except Exception as e:
        handle_error(e)
        logger.error(f"Exception in listen: {e}")
        show_error("A problem occurred in the listener. See logs for details.")


if __name__ == "__main__":
    main()
