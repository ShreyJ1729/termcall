# TermCall: FaceTime in the terminal

todo - in order to fix problem of force exit not removing name from database, create background process that monitors heartbeat from main program every second. If heartbeat is not received, remove name from database and exit.

todo - since high resolution causes high latency, have some way to auto balance latency once it gets too bad.
e.x. force lower resolution even on high res terminal screens if latency is too high.

todo - stream events to keep download data low on firebase
todo - separate the camera struct into framereader and framewriter. this allows us to bypass acquiring the camera lock which is the main bottleneck in the program.

bottlenecks

- reading frame (occassionaly on send side)
