use crate::schemas::user::User;
use anyhow::Result;
use firebase_rs::Firebase;
use std::collections::HashMap;

// The .json at the end allows for receiving changes through a stream
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
        let users = self
            .firebase
            .at("users")
            .get::<HashMap<String, User>>()
            .await
            .unwrap_or(HashMap::new());
        users
    }

    pub async fn get_usernames(&self) -> Vec<String> {
        let users = self.get_users().await;
        users.keys().cloned().collect()
    }
    pub async fn add_or_update_user(&self, username: &str, new_data: User) -> Result<()> {
        self.firebase
            .at("users")
            .at(username)
            .update(&new_data)
            .await?;
        Ok(())
    }

    pub async fn remove_user(&self, username: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.firebase.at("users").at(username).delete().await?;
        Ok(())
    }
}
