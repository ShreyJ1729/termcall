import os
import requests
import threading

FIREBASE_DB_URL = "https://termcall-a14ab-default-rtdb.firebaseio.com"


class FirebaseRef:
    def __init__(self, path):
        self.path = path
        self.lock = threading.Lock()

    def _url(self, subpath=None):
        url = f"{FIREBASE_DB_URL}/{self.path}"
        if subpath:
            url += f"/{subpath}"
        return url + ".json"

    def get(self, subpath=None):
        url = self._url(subpath)
        resp = requests.get(url)
        resp.raise_for_status()
        return resp.json()

    def set(self, value, subpath=None):

        url = self._url(subpath)
        resp = requests.put(url, json=value)
        resp.raise_for_status()
        return resp.json()

    def update(self, value, subpath=None):
        url = self._url(subpath)
        resp = requests.patch(url, json=value)
        resp.raise_for_status()
        return resp.json()

    def push(self, value):
        url = self._url()
        resp = requests.post(url, json=value)
        resp.raise_for_status()
        result = resp.json()
        key = result.get("name")
        return FirebasePushedRef(self, key)

    def child(self, subpath):
        return FirebaseRef(f"{self.path}/{subpath}")


class FirebasePushedRef:
    def __init__(self, parent_ref, key):
        self.parent_ref = parent_ref
        self.key = key

    @property
    def key(self):
        return self._key

    @key.setter
    def key(self, value):
        self._key = value

    def get(self):
        return self.parent_ref.get(self.key)

    def set(self, value):
        return self.parent_ref.set(value, self.key)

    def update(self, value):
        return self.parent_ref.update(value, self.key)

    def child(self, subpath):
        return self.parent_ref.child(f"{self.key}/{subpath}")


users_ref = FirebaseRef("users")
call_requests_ref = FirebaseRef("call_requests")
signaling_ref = FirebaseRef("signaling")


def clear_screen():
    os.system("cls" if os.name == "nt" else "clear")
