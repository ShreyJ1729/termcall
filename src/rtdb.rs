use crate::schemas::user::User;
use anyhow::Result;
use firebase_rs::Firebase;
use simple_log::{error, warn};
use std::collections::HashMap;

pub const DATABASE_URL: &str = "https://termcall-a14ab-default-rtdb.firebaseio.com/.json";

pub struct RTDB {
    firebase: Firebase,
}

impl RTDB {
    pub fn new() -> RTDB {
        let firebase = Firebase::new(DATABASE_URL).unwrap();
        RTDB { firebase }
    }

    pub async fn get_users(&self) -> HashMap<String, User> {
        match self
            .firebase
            .at("users")
            .get::<HashMap<String, User>>()
            .await
        {
            Ok(users) => users,
            Err(_) => {
                warn!("Could not get users from database. Assuming no users and returning empty hashmap.");
                HashMap::new()
            }
        }
    }

    pub async fn get_usernames(&self) -> Vec<String> {
        let users = self.get_users().await;
        users.keys().cloned().collect()
    }

    pub async fn add_or_update_user(&self, username: &str, new_data: User) -> Result<()> {
        match self
            .firebase
            .at("users")
            .at(username)
            .update(&new_data)
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => {
                error!("could not update user {}", username);
                Err(anyhow::anyhow!("could not update user {}", username))
            }
        }
    }

    pub async fn remove_user(&self, username: &str) {
        match self.firebase.at("users").at(username).delete().await {
            Ok(_) => {}
            Err(_) => {
                error!("could not delete user {}", username);
            }
        }
    }
}
