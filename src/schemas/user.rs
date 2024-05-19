use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub name: String,
    pub offer: String,
    pub answer: String,
    pub in_call: String,
    pub sending_call: String,
    pub receiving_call: String,
}

impl User {
    pub fn new(name: String) -> User {
        User {
            name,
            offer: String::new(),
            answer: String::new(),
            in_call: String::new(),
            sending_call: String::new(),
            receiving_call: String::new(),
        }
    }
}
