use crate::rtdb::{self, RTDB};
use anyhow::Result;
use crossterm::event;
use firebase_rs::Firebase;
use std::io::stdin;

pub fn render_homescreen(usernames: &mut Vec<String>, self_name: &str) {
    println!(
        "Welcome, {}! This is your dashboard. If anyone calls you, you'll get a notification here. If you want to call someone, enter their name below.\nNames are case sensitive\n",
        self_name
    );
    usernames.sort();
    usernames
        .iter()
        .filter(|username| username != &self_name)
        .for_each(|contact| {
            println!("{}", contact);
        });
}

pub fn handle_homescreen_input(usernames: &Vec<String>, person_to_call: &mut String) -> bool {
    if let event::Event::Key(event) = event::read().unwrap() {
        match event.code {
            event::KeyCode::Enter => {
                if usernames.contains(&person_to_call) {
                    return true;
                }
                println!("That person is not in your contacts. Try again.");
                person_to_call.clear();
            }
            event::KeyCode::Backspace => {
                person_to_call.pop();
            }
            event::KeyCode::Char(c) => {
                person_to_call.push(c);
            }
            _ => {}
        }
    }
    false
}

pub async fn wait_get_name(rtdb: &RTDB) -> Result<String> {
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
