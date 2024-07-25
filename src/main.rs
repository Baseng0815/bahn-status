use std::{error::Error, io::stdout, time::Duration};

use frontend::Frontend;
use ratatui::crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

mod api;
mod frontend;

fn main() -> Result<(), Box<dyn Error>> {
    // let endpoints = ApiEndpoints {
    //     status: String::from("https://iceportal.de/api1/rs/status"),
    //     trip: String::from("https://iceportal.de/api1/rs/tripInfo/trip"),
    // };

    // let info = Info::query(&endpoints)?;

    let tick_rate = Duration::from_millis(1000); // update every second

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut frontend = Frontend::new(50)?;
    frontend.enter_loop(tick_rate)?;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
