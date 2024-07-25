use std::{collections::VecDeque, error::Error, io::{self, stdout, Stdout}, path::PathBuf, time::{Duration, Instant}};

use chrono::{DateTime, Local, NaiveDateTime};
use ratatui::{
    backend::CrosstermBackend, crossterm::{event::{self, Event, KeyCode}, terminal::{enable_raw_mode, EnterAlternateScreen}, ExecutableCommand}, layout::{Constraint, Direction, Layout, Rect}, style::Color, text::{Line, Span, Text}, widgets::{self, canvas::{Canvas, Circle, Map, MapResolution, Shape}, Block, Paragraph}, Frame, Terminal
};

use crate::api::{ApiPaths, Info, Station, StatusInfo};

// +- Status information --------------------------
// | Current Speed:      113
// | Connectivity:       HIGH
// | Total trip length:  746km
// | Traveled so far:    73km
// | Remaining:          339km
// | Distance to next:   21km (Friedberg (Hess))
// | Latitude/longitude: (50.57N, 8.66W)
// +-----------------------------------------------


#[derive(Debug, PartialEq)]
enum PanelSelection {
    BasicInformation,
    StatusInformation,
    SpeedInformation,
    TripInformation,
}

impl PanelSelection {
    pub fn next(&mut self) {
        *self = match *self {
            PanelSelection::BasicInformation => PanelSelection::StatusInformation,
            PanelSelection::StatusInformation => PanelSelection::SpeedInformation,
            PanelSelection::SpeedInformation => PanelSelection::TripInformation,
            PanelSelection::TripInformation => PanelSelection::BasicInformation,
        }
    }

    pub fn prev(&mut self) {
        *self = match *self {
            PanelSelection::BasicInformation => PanelSelection::TripInformation,
            PanelSelection::StatusInformation => PanelSelection::BasicInformation,
            PanelSelection::SpeedInformation => PanelSelection::StatusInformation,
            PanelSelection::TripInformation => PanelSelection::SpeedInformation,
        }
    }
}

// variables preserved across draw calls
#[derive(Debug)]
pub struct Frontend {
    selection: PanelSelection,
    data: VecDeque<Info>, // server timestamp contained in status
}

impl Frontend {
    pub fn new(bufsize: usize) -> Result<Frontend, Box<dyn Error>> {
        Ok(Frontend {
            selection: PanelSelection::BasicInformation,
            data: VecDeque::with_capacity(bufsize),
        })
    }

    fn draw_basic_info(&self, frame: &mut Frame, area: Rect) {
        let info = self.data.back().expect("Nothing to draw");

        let content = format!("\
Schienenfahrzeugtyp:           {}
Schienenfahrzeugbezeichnung:   {}
Sozioökonomisches Milieu:      {}
Streckenführung:               von {} nach {}
", info.status.trainType, info.status.tzn, info.status.wagonClass,
info.trip.trip.stops.first().expect("Everything has to start somewhere").station.name,
info.trip.trip.stops.last().expect("Everything has to end somewhere").station.name);

        let block = if self.selection == PanelSelection::BasicInformation {
            Block::bordered().title("Grundlegende Informationen").border_style(Color::Magenta)
        } else {
            Block::bordered().title("Grundlegende Informationen")
        };

        frame.render_widget(Paragraph::new(content).block(block), area);
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let info = self.data.back().expect("Nothing to draw");

        let ap = info.trip.trip.actualPosition;
        let td = info.trip.trip.totalDistance;

        let average_speed = self.data.iter().fold(0.0, |acc, e| acc + e.status.speed) / self.data.len() as f64;

        let content = format!("\
Aktuelle Geschwindigkeit:      {:.0}km/h
   Gleitender Mittelwert:      {:.0}km/h
Internetzwerkverbindungsgüte:  {}
Gesamte Streckenlänge:         {}km
Davon bereits zurückgelegt:    {}km ({:.2}%)
Verbleibend (nach Adam Riese): {}km ({:.2}%)
Entfernung zum nächsten Halt:  {}km ({})
Aktuelle geographische Lage:   ({:.03}N, {:.03}W)",
info.status.speed, average_speed, info.status.internet, td / 1000, ap / 1000, ap as f64 / td as f64 * 100.0,
(td - ap) / 1000, (td - ap) as f64 / td as f64 * 100.0, 0, "NEXT STOP", info.status.latitude, info.status.longitude);

        let block = if self.selection == PanelSelection::StatusInformation {
            Block::bordered().title("Statusinformation").border_style(Color::Magenta)
        } else {
            Block::bordered().title("Statusinformation")
        };

        frame.render_widget(Paragraph::new(content).block(block), area);
    }

    fn draw_speed_graph(&self, frame: &mut Frame, area: Rect) {
        let block = if self.selection == PanelSelection::SpeedInformation {
            Block::bordered().title("Geschwindigkeitsverlauf").border_style(Color::Magenta)
        } else {
            Block::bordered().title("Geschwindigkeitsverlauf")
        };

        let canvas = Canvas::default()
            .block(block)
            .x_bounds([0.0, self.data.capacity() as f64])
            .y_bounds([0.0, 300.0])
            .paint(|ctx| {
                for (xc, (curr, next)) in self.data.iter().zip(self.data.iter().skip(1)).enumerate() {
                    ctx.draw(&widgets::canvas::Line {
                        x1: xc as f64,
                        y1: curr.status.speed,
                        x2: xc as f64 + 1.0,
                        y2: next.status.speed,
                        color: if curr.status.speed >= next.status.speed { Color::Red } else { Color::Green }
                    });
                }
            });

        frame.render_widget(canvas, area);
    }

    // struct TripShape {
    //     stations: Vec<Station>
    // }

    // impl Shape for TripShape {
    //     fn draw(&self, painter: &mut ratatui::widgets::canvas::Painter) {
    //         painter.li
    //     }
    // }

    fn draw_trip(&self, frame: &mut Frame, area: Rect) {
        let info = self.data.back().expect("Nothing to draw");

        let lphk = 5; // lines per kilometers (TODO calculate appropriate value)

        let height = (area.height - 2) as usize; // subtract 2 for border

        let (mut miny, mut maxy, mut minx, mut maxx) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for stop in &info.trip.trip.stops {
            miny = stop.station.geocoordinates.latitude.min(miny);
            maxy = stop.station.geocoordinates.latitude.max(maxy);
            minx = stop.station.geocoordinates.longitude.min(minx);
            maxx = stop.station.geocoordinates.longitude.max(maxx);
        }

        let data_when: DateTime<Local> = DateTime::from_timestamp(info.status.serverTime as i64, 0).unwrap().into();
        let now = Local::now().time();
        let diff = now - data_when.time();

        let block = if self.selection == PanelSelection::TripInformation {
            Block::bordered().title("Streckenverlauf").border_style(Color::Magenta)
                .title_bottom(format!("[Zuletzt aktualisiert: {} (vor {} Sekunden)]", data_when.format("%H:%M:%S"), diff.num_seconds()))
        } else {
            Block::bordered().title("Streckenverlauf")
                .title_bottom(format!("[Zuletzt aktualisiert: {} (vor {} Sekunden)]", data_when.format("%H:%M:%S"), diff.num_seconds()))
        };

        let canvas = Canvas::default()
            .block(block)
            .x_bounds([minx, maxx])
            .y_bounds([miny, maxy])
            .paint(|ctx| {
                for (curr, next) in info.trip.trip.stops.iter().zip(info.trip.trip.stops.iter().skip(1)) {
                    ctx.draw(&widgets::canvas::Line {
                        x1: curr.station.geocoordinates.longitude,
                        y1: curr.station.geocoordinates.latitude,
                        x2: next.station.geocoordinates.longitude,
                        y2: next.station.geocoordinates.latitude,
                        color: Color::White,
                    });

                    let text = if let Some(sat) = curr.timetable.scheduledArrivalTime {
                        let time: DateTime<Local> = DateTime::from_timestamp(sat as i64 / 1000, 0).unwrap().into();
                        let now = Local::now().time();
                        let aat = curr.timetable.actualArrivalTime.expect("If there is a scheduled time there should also be an actual time");
                        let delay = (aat as i64 - sat as i64) / 1000 / 60;

                        let delay_mood = match delay {
                            -1000..0 => "🤨",
                            0..1 => "😁",
                            1..2 => "😄",
                            2..4 => "😃",
                            4..6 => "😀",
                            6..9 => "🤔",
                            9..13 => "🫠",
                            13..18 => "🥲",
                            18..30 => "😨",
                            30..40 => "🫢",
                            40..60 => "😬",
                            60..80 => "🫨",
                            80..100 => "🤮",
                            100..120 => "🤯",
                            120..140 => "🤬",
                            _ => "💀",
                        };

                        if delay == 0 {
                            format!("{} ({})", curr.station.name.clone(), time.format("%H:%M"))
                        } else {
                            format!("{} ({}; {}{}{})", curr.station.name.clone(), time.format("%H:%M"),
                            if delay < 0 { "-" } else { "+" }, delay, delay_mood)
                        }
                    } else {
                        format!("{} (-)", curr.station.name.clone())
                    };

                    ctx.print(curr.station.geocoordinates.longitude, curr.station.geocoordinates.latitude, Line::from(text));
                }

                ctx.draw(&Circle {
                    x: info.trip.trip.stops[3].station.geocoordinates.longitude,
                    y: info.trip.trip.stops[3].station.geocoordinates.latitude,
                    radius: 0.01,
                    color: Color::Red,
                });
            });

        frame.render_widget(canvas, area);
    }

    fn ui(&self, frame: &mut Frame) {
        let layout = Layout::new(Direction::Vertical, [ Constraint::Length(6), Constraint::Length(10), Constraint::default() ])
            .split(frame.size());

        let layout_1 = Layout::new(Direction::Horizontal, [ Constraint::Min(50), Constraint::default() ])
            .split(layout[1]);

        self.draw_basic_info(frame, layout[0]);
        self.draw_status(frame, layout_1[0]);
        self.draw_speed_graph(frame, layout_1[1]);
        self.draw_trip(frame, layout[2]);
    }

    // update state (query API, move graphs, ...)
    fn tick(&mut self) {
        let files = ApiPaths {
            status: PathBuf::from("sample/status.json"),
            trip: PathBuf::from("sample/trip.json"),
        };

        let info = Info::from_file(&files).unwrap();

        if self.data.len() == self.data.capacity() {
            self.data.pop_front();
        }

        self.data.push_back(info);
    }

    pub fn enter_loop(&mut self, tick_rate: Duration) -> io::Result<bool> {
        let mut last_tick = Instant::now();
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        self.tick(); // tick once to initialize

        loop {
            terminal.draw(|frame| self.ui(frame))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == event::KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => { return Ok(true); }
                            KeyCode::Tab => { self.selection.next(); }
                            KeyCode::BackTab => { self.selection.prev(); }
                            _ => (),
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
                self.tick();
            }
        }
    }
}
