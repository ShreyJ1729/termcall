# TermCall: FaceTime in the terminal

in order to fix problem of force exit not removing name from database, create background process that monitors heartbeat from main program every second. If heartbeat is not received, remove name from database and exit.
