import pyrebase

# Extracted from serviceAccountKey.json and Firebase console
firebase_config = {
    "apiKey": "AIzaSyBJtaLY84jOGFN4WPUjkVO6_HyIDMw0SNQ",
    "authDomain": "termcall-a14ab.firebaseapp.com",
    "databaseURL": "https://termcall-a14ab-default-rtdb.firebaseio.com/",
    "projectId": "termcall-a14ab",
    "storageBucket": "termcall-a14ab.appspot.com",
    "messagingSenderId": "111430797061287841762",
    "appId": "1:903903218297:web:d3f07722d5197ebb5ee2a8",
}

_firebase = None


def get_firebase():
    global _firebase
    if _firebase is None:
        _firebase = pyrebase.initialize_app(firebase_config)
    return _firebase, _firebase.auth(), _firebase.database()


def get_all_user_profiles(id_token):
    """Fetch all user profiles from RTDB /users. For directory browsing only, not authentication."""
    _, _, db = get_firebase()
    users = db.child("users").get(id_token)
    if not users.each():
        return []
    return [dict(uid=user.key(), **user.val()) for user in users.each()]
