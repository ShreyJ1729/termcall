use crate::rtdb::{self, RTDB};
use crossterm::event;
use firebase_rs::Firebase;
use std::io::stdin;

pub fn render_contacts(usernames: &mut Vec<String>, self_name: &str) {
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

pub fn handle_input_home_screen(usernames: &Vec<String>, person_to_call: &mut String) -> bool {
    if event::poll(std::time::Duration::from_millis(50)).unwrap() {
        if let event::Event::Key(event) = event::read().unwrap() {
            if event.code == event::KeyCode::Enter {
                if usernames.contains(&person_to_call) {
                    return true;
                }
                println!("That person is not in your contacts. Try again.");
                person_to_call.clear();
            } else if event.code == event::KeyCode::Backspace {
                if person_to_call.len() > 0 {
                    person_to_call.pop();
                }
            } else if let event::KeyCode::Char(c) = event.code {
                person_to_call.push(c);
            }
        }
    }

    return false;
}

pub async fn wait_get_name(rtdb: &RTDB) -> String {
    let mut self_name = String::new();
    loop {
        stdin()
            .read_line(&mut self_name)
            .expect("Failed to read line");
        self_name = self_name.trim().to_string();

        let usernames = rtdb.get_usernames().await;
        if usernames.contains(&self_name) {
            println!("User already exists. Try entering a different name: ");
            self_name.clear();
            continue;
        }
        break;
    }

    return self_name;
}
