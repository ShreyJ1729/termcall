use crate::rtdb::{self, RTDB};
use anyhow::Result;
use crossterm::event::{self, KeyEventKind};
use firebase_rs::Firebase;
use std::io::stdin;

pub async fn wait_get_unique_name(rtdb: &RTDB) -> Result<String> {
    let mut self_name = String::new();
    loop {
        stdin().read_line(&mut self_name)?;
        self_name = self_name.trim().to_string();

        let usernames = rtdb.get_usernames().await;
        match usernames.contains(&self_name) {
            true => {
                println!("User already exists. Try entering a different name: ");
                self_name.clear();
            }
            false => break,
        }
    }

    Ok(self_name)
}
