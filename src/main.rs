use std::{error::Error, io::stdout, path::PathBuf};

use api::{ApiPaths, Info};
use frontend::ui;
use ratatui::{backend::CrosstermBackend, crossterm::{terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand}, Terminal};

use crate::{api::{ApiEndpoints, StatusInfo, TripInfo}, frontend::handle_events};

mod api;
mod frontend;

fn main() -> Result<(), Box<dyn Error>> {
    // let endpoints = ApiEndpoints {
    //     status: String::from("https://iceportal.de/api1/rs/status"),
    //     trip: String::from("https://iceportal.de/api1/rs/tripInfo/trip"),
    // };

    // let info = Info::query(&endpoints)?;

    // frontend loop
    enable_raw_mode();
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut should_quit = false;
    while !should_quit {
        terminal.draw(ui)?;
        should_quit = handle_events()?;
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
