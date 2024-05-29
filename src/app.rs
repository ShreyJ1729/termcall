use std::{collections::HashMap, io};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    prelude::*,
    symbols::border,
    widgets::{block::*, *},
    Frame,
};

use crate::{handle_incoming_call, rtc::PeerConnection, rtdb::RTDB, schemas::user::User, tui};

#[derive(Debug, Default)]
pub struct App {
    contacts: HashMap<String, User>,
    selected: usize,
    exit: bool,
}

impl App {
    /// runs the application's main loop until the user quits
    pub async fn run(&mut self, terminal: &mut tui::Tui, self_name: &str) -> anyhow::Result<()> {
        let rtc_connection = PeerConnection::new().await?;
        let rtdb = RTDB::new();
        let mut begin = std::time::Instant::now();

        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;

            if begin.elapsed().as_millis() > 500 {
                self.update_contacts(&rtdb).await;
                begin = std::time::Instant::now();
            }

            // Check if anyone is calling us (someone else's sending_call is our name)
            let potential_caller = self
                .contacts
                .iter()
                .find(|(_k, v)| v.sending_call == self_name);

            if let Some((_, caller_data)) = potential_caller {
                handle_incoming_call(&self_name, caller_data, &rtdb, &rtc_connection).await?;
            }
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.size());
    }

    async fn update_contacts(&mut self, rtdb: &RTDB) {
        self.contacts = rtdb.get_users().await;
    }

    fn contact_names(&self) -> Vec<String> {
        let mut list = self.contacts.keys().cloned().collect::<Vec<String>>();
        list.sort();
        list
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            }
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.exit = true,
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(self.contacts.len() - 1),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Title::from(" TermCall ".bold());
        let instructions = Title::from(Line::from(vec![
            " ↑/↓ ".into(),
            "<Up/Down>".blue().bold(),
            " Select ".into(),
            "<Enter>".blue().bold(),
            " Quit ".into(),
            "<Esc> ".blue().bold(),
        ]));

        let block = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Center)
                    .position(Position::Bottom),
            )
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .padding(Padding::top(2))
            .padding(Padding::horizontal(4));

        // make the contact at selected index bold
        let cnames = self.contact_names();
        let contacts_text = cnames.iter().enumerate().map(|(i, name)| {
            let name = if i == self.selected {
                (">".to_owned() + name).yellow()
            } else {
                (" ".to_owned() + name).into()
            };
            name
        });

        let list = List::new(contacts_text).block(block);

        ratatui::widgets::Widget::render(&list, area, buf);
    }
}
