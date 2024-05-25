# TermCall: FaceTime in the terminal

problems to fix

- force exit not removing name from database, create background process (daemon) that monitors heartbeat from main program every second. If heartbeat is not received, remove name from database and exit.

- high latency issue is caused by two reasons
- - mutex on data channel takes too much time to lock/unlock, causing delays in transmitting/recieving loop. high output rate causes lag also. for 1, figure out different data channels. for two,
    have some way to auto balance latency once it gets too bad.
    e.x. force lower resolution even on high res terminal screens if latency is too high.

- firebase download rate is way too high. subscribe to receive events from stream instead of polling every second.
