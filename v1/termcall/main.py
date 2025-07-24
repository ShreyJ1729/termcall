# TODO: Move this file to termcall/main.py and split into modules for pip packaging.
"""
TermCall main CLI entry point.
Importable as a module for pip packaging.
"""

import asyncio
import json
import os
import uuid
from pathlib import Path
import firebase_admin
from firebase_admin import credentials, db, initialize_app
from prompt_toolkit import PromptSession
from prompt_toolkit.completion import WordCompleter
from prompt_toolkit.shortcuts import CompleteStyle
from prompt_toolkit.shortcuts import button_dialog
from aiortc import (
    RTCPeerConnection,
    RTCSessionDescription,
    RTCIceCandidate,
    VideoStreamTrack,
    MediaStreamTrack,
)
from aiortc.contrib.media import MediaPlayer
import cv2
import numpy as np
import time
from image_to_ascii import ImageToAscii
from sixel.converter import SixelConverter
import sys
from pynput import keyboard
import threading

from .auth import authenticate, view_profile
from .webrtc import initiate_call_request, listen_for_incoming_calls
from .firebase import clear_screen
from prompt_toolkit import PromptSession

__all__ = ["cli_main", "main"]


async def settings_menu():
    clear_screen()
    print("\n--- Settings ---")
    print("(Settings functionality coming soon!)")
    print("----------------\n")
    import asyncio

    await asyncio.sleep(0.5)


async def main_menu(email, full_name):
    session = PromptSession()
    while True:
        clear_screen()
        print("\n=== TermCall Main Menu ===")
        print(f"Logged in as: {full_name} ({email})")
        print("1. Browse users to call")
        print("2. View profile")
        print("3. Settings")
        print("4. Exit")
        print("5. Test call with yourself (loopback)")
        try:
            choice = await session.prompt_async("Select an option [1-5]: ")
        except (EOFError, KeyboardInterrupt):
            print("\nExiting...")
            return
        if choice == "1":
            # Import browse_users_for_call from webrtc if needed
            from .webrtc import browse_users_for_call

            selected = await browse_users_for_call(email)
            if selected:
                callee_email, callee_name = selected
                print(f"Ready to initiate call to {callee_name} ({callee_email})")
                from .webrtc import initiate_call_request

                accepted = await initiate_call_request(email, callee_email)
                if accepted:
                    print("Proceed to WebRTC signaling and media exchange (step 8)...")
                else:
                    print("Call not established.")
            input("Press Enter to return to the main menu...")
        elif choice == "2":
            await view_profile(email)
            input("Press Enter to return to the main menu...")
        elif choice == "3":
            await settings_menu()
            input("Press Enter to return to the main menu...")
        elif choice == "4":
            print("Goodbye!")
            return
        elif choice == "5":
            print("\n[Loopback Test] Initiating a call to yourself...")
            from .firebase import call_requests_ref
            from .webrtc import handle_call_flow
            import time

            call_id = None
            # Create a call request as in initiate_call_request
            request = {
                "caller_email": email,
                "callee_email": email,
                "status": "pending",
                "timestamp": int(time.time()),
            }
            new_ref = call_requests_ref.push(request)
            call_id = new_ref.key

            # Define callee coroutine that auto-accepts
            async def auto_accept_callee():
                # Wait for the call request to appear
                while True:
                    req = call_requests_ref.child(call_id).get()
                    if req and req.get("status") == "pending":
                        break
                    await asyncio.sleep(0.2)
                # Auto-accept
                call_requests_ref.child(call_id).update({"status": "accepted"})
                print("[Loopback] Callee auto-accepted call.")
                await handle_call_flow(False, email, email, call_id, loopback=True)

            # Start callee first, then caller after short delay
            callee_task = asyncio.create_task(auto_accept_callee())
            await asyncio.sleep(0.1)
            caller_task = asyncio.create_task(
                handle_call_flow(True, email, email, call_id, loopback=True)
            )
            await asyncio.gather(caller_task, callee_task)
            print("Loopback call completed.")
            input("Press Enter to return to the main menu...")
        else:
            print("Invalid option. Please select 1-5.")


async def main():
    email, full_name = await authenticate()
    print(f"Welcome, {full_name}!")
    # Start listening for incoming calls in the background
    listener_task = asyncio.create_task(listen_for_incoming_calls(email))
    await main_menu(email, full_name)
    # Cancel the listener when exiting
    listener_task.cancel()
    try:
        await listener_task
    except asyncio.CancelledError:
        pass


def cli_main():
    import asyncio

    asyncio.run(main())


def __main_entry():
    cli_main()


if __name__ == "__main__":
    __main_entry()
