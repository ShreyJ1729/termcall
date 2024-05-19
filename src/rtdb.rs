use crate::schemas::user::User;
use anyhow::Result;
use firebase_rs::Firebase;
use std::collections::HashMap;

// The .json at the end allows for receiving changes through a stream
pub const DATABASE_URL: &str = "https://termcall-a14ab-default-rtdb.firebaseio.com/.json";

pub async fn get_users(firebase: &Firebase) -> Result<HashMap<String, User>> {
    let users = firebase.at("users").get_as_string().await?;
    let users = serde_json::from_str::<HashMap<String, User>>(&users.data)?;
    Ok(users)
}

pub async fn get_usernames(firebase: &Firebase) -> Result<Vec<String>> {
    let users = get_users(firebase).await?;
    Ok(users.keys().cloned().collect())
}

pub async fn add_or_update_user(firebase: &Firebase, username: &str, new_data: User) -> Result<()> {
    firebase.at("users").at(username).update(&new_data).await?;
    Ok(())
}
