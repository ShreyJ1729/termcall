use crate::rtdb::RTDB;
use anyhow::Result;
use std::io::{self, stdin, Write};

pub async fn wait_get_unique_name(rtdb: &RTDB) -> Result<String> {
    let mut self_name = String::new();
    loop {
        stdin().read_line(&mut self_name)?;
        self_name = self_name.trim().to_string();
        if self_name == "" {
            print!("Name cannot be empty. Try entering a different name: ");
            io::stdout().flush()?;
            continue;
        }

        let usernames = rtdb.get_usernames().await;
        match usernames.contains(&self_name) {
            true => {
                print!("User already exists. Try entering a different name: ");
                io::stdout().flush()?;
                self_name.clear();
            }
            false => break,
        }
    }

    Ok(self_name)
}
