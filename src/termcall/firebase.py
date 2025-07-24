import os
import firebase_admin
from firebase_admin import credentials, db

_firebase_app = None


def init_firebase():
    global _firebase_app
    if _firebase_app is not None:
        return _firebase_app
    cred_path = os.environ.get("FIREBASE_SERVICE_ACCOUNT", "firebase-key.json")
    if not os.path.exists(cred_path):
        raise FileNotFoundError(f"Firebase service account file not found: {cred_path}")
    try:
        cred = credentials.Certificate(cred_path)
        _firebase_app = firebase_admin.initialize_app(
            cred,
            {
                "databaseURL": os.environ.get(
                    "FIREBASE_DATABASE_URL", "https://your-project-id.firebaseio.com"
                )
            },
        )
        return _firebase_app
    except Exception as e:
        raise RuntimeError(f"Failed to initialize Firebase Admin SDK: {e}")


def get_db():
    if not firebase_admin._apps:
        init_firebase()
    return db
