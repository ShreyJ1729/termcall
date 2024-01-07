# TermCall: FaceTime in the terminal

## Feature Roadmap

- look more into difference between libsixel and libsixel-sys --> if needed, create own FFI bindings
- simple `brew install termcall` for installation
- login with google
- uses webrtc for video/audio calls. makes group calls easy
- see which friends are active/inactive/in call (green dot, red dot, yellow dot)
- in-call file transfer
- ascii graphics for non-sixel terminals
- can easily control video/audio settings for good performance
- figure out some way to optimize sixel rendering (ideal = 1920x1080 30fps)
- fix the terminal buffer size issue (some clever trick with alternate screen required)
