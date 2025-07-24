# TermCall

A CLI-based video/audio calling app for the terminal using WebRTC and Firebase.

## Features

- Peer-to-peer video and audio calls in your terminal
- Remote video as colored ASCII art
- Local video preview as Sixel graphics (Sixel-compatible terminal required)
- Simple CLI interface for browsing users and making calls

## Requirements

- Python 3.8+
- A Sixel-compatible terminal (e.g., xterm -ti vt340, mlterm) for local video preview
- Webcam and microphone

## Installation

1. **Install TermCall (after building the package):**

   ```bash
   pip install termcall
   ```

2. **Install system dependencies (if needed):**

   - macOS: `brew install libjpeg`
   - Ubuntu: `sudo apt-get install libopencv-dev`

## Usage

After installation, run:

```bash
termcall
```

- On first run, you will be prompted for your email and full name.
- Browse users, initiate calls, and accept/decline incoming calls from the CLI menu.
- During a call:
  - Remote video is shown as ASCII art
  - Local video preview (bottom right) uses Sixel graphics (if supported)
  - Controls: `m` to mute audio, `v` to mute video, `q` to quit call
