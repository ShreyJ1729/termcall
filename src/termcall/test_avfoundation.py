import sys
from aiortc.contrib.media import MediaPlayer


def try_device(device_string, options=None):
    try:
        print(f"Trying device string: {device_string} with options: {options}")
        player = MediaPlayer(device_string, format="avfoundation", options=options)
        # Try to get a frame to force open the device
        frame = next(player.video)
        print(f"SUCCESS: Opened {device_string} with options {options}")
        return True
    except Exception as e:
        print(f"ERROR: {device_string} with options {options}: {e}")
        return False


def main():
    # Video devices: 0 (FaceTime HD Camera), 1 (Capture screen 0)
    # Audio devices: 0 (Background Music), 1 (MacBook Pro Microphone), 2 (Background Music (UI Sounds)), 3 (Microsoft Teams Audio)
    device_strings = [
        "avfoundation:0:0",  # FaceTime HD Camera + Background Music
        "avfoundation:0:1",  # FaceTime HD Camera + MacBook Pro Microphone
        "avfoundation:0:2",  # FaceTime HD Camera + Background Music (UI Sounds)
        "avfoundation:0:3",  # FaceTime HD Camera + Microsoft Teams Audio
        "avfoundation:1:0",  # Capture screen 0 + Background Music
        "avfoundation:1:1",  # Capture screen 0 + MacBook Pro Microphone
        "avfoundation:1:2",  # Capture screen 0 + Background Music (UI Sounds)
        "avfoundation:1:3",  # Capture screen 0 + Microsoft Teams Audio
        "avfoundation:0",  # FaceTime HD Camera only
        "avfoundation:1",  # Capture screen 0 only
    ]
    options = {"framerate": "30", "video_size": "640x480"}
    any_success = False
    for dev in device_strings:
        if try_device(dev, options=options):
            any_success = True
            break
    if not any_success:
        print("\nAll device string attempts failed. See errors above.")
        sys.exit(1)


if __name__ == "__main__":
    main()
