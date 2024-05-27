use crate::schemas::user::User;
use firebase_rs::Firebase;
use std::collections::HashMap;

// The .json at the end allows for receiving changes through a stream
pub const DATABASE_URL: &str = "https://termcall-a14ab-default-rtdb.firebaseio.com/.json";

pub async fn get_users(firebase: &Firebase) -> HashMap<String, User> {
    let users = firebase
        .at("users")
        .get::<HashMap<String, User>>()
        .await
        .unwrap_or(HashMap::new());
    users
}

pub async fn get_usernames(firebase: &Firebase) -> Vec<String> {
    let users = get_users(firebase).await;
    users.keys().cloned().collect()
}
pub async fn add_or_update_user(
    firebase: &Firebase,
    username: &str,
    new_data: User,
) -> Result<(), Box<dyn std::error::Error>> {
    firebase.at("users").at(username).update(&new_data).await?;
    Ok(())
}

pub async fn remove_user(
    firebase: &Firebase,
    username: &str,
) -> Result<(()), Box<dyn std::error::Error>> {
    firebase.at("users").at(username).delete().await?;
    Ok(())
}
