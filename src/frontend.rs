use std::{
    collections::VecDeque,
    error::Error,
    io::{self, stdout, Stdout},
    path::PathBuf,
    time::{Duration, Instant},
};

use chrono::{DateTime, Local, NaiveDateTime};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode},
        terminal::{enable_raw_mode, EnterAlternateScreen},
        ExecutableCommand,
    },
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
    text::{Line, Span, Text},
    widgets::{
        self,
        canvas::{Canvas, Circle, Map, MapResolution, Shape},
        Block, Paragraph,
    },
    Frame, Terminal,
};

use crate::api::{ApiEndpoints, ApiPaths, Info, Station, StatusInfo};

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

    // data for trip information
    selected_station_detailed: bool,
    selected_station: usize,
}

impl Frontend {
    pub fn new(bufsize: usize) -> Result<Frontend, Box<dyn Error>> {
        Ok(Frontend {
            selection: PanelSelection::BasicInformation,
            data: VecDeque::with_capacity(bufsize),
            selected_station_detailed: false,
            selected_station: 0,
        })
    }

    fn draw_basic_info(&self, frame: &mut Frame, area: Rect) {
        let info = self.data.back().expect("Nothing to draw");

        let content = format!(
            "\
Schienenfahrzeugtyp:           {}
Schienenfahrzeugbezeichnung:   {}
Sozioökonomisches Milieu:      {}
Streckenführung:               von {} nach {}
",
            info.status.trainType,
            info.status.tzn,
            info.status.wagonClass,
            info.trip
                .trip
                .stops
                .first()
                .expect("Everything has to start somewhere")
                .station
                .name,
            info.trip
                .trip
                .stops
                .last()
                .expect("Everything has to end somewhere")
                .station
                .name
        );

        let block = if self.selection == PanelSelection::BasicInformation {
            Block::bordered()
                .title("Grundlegende Informationen")
                .border_style(Color::Magenta)
        } else {
            Block::bordered().title("Grundlegende Informationen")
        };

        frame.render_widget(Paragraph::new(content).block(block), area);
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let info = self.data.back().expect("Nothing to draw");

        let ap = info.trip.trip.actualPosition;
        let td = info.trip.trip.totalDistance;

        let average_speed =
            self.data.iter().fold(0.0, |acc, e| acc + e.status.speed) / self.data.len() as f64;

        let next_stop_eva = &info.trip.trip.stopInfo.scheduledNext;
        let next_stop = &info
            .trip
            .trip
            .stops
            .iter()
            .find(|stop| &stop.station.evaNr == next_stop_eva)
            .expect("A stop with this evaNr must exist");

        let next_stop_dist = next_stop.info.distanceFromStart - ap;
        let next_stop_name = &next_stop.station.name;

        let content = format!(
            "\
Aktuelle Geschwindigkeit:      {:.0}km/h
   Gleitender Mittelwert:      {:.0}km/h
Internetzwerkverbindungsgüte:  {}
Gesamte Streckenlänge:         {}km
Davon bereits zurückgelegt:    {}km ({:.2}%)
Verbleibend (nach Adam Riese): {}km ({:.2}%)
Entfernung zum nächsten Halt:  {}km ({})
Aktuelle geographische Lage:   ({:.03}N, {:.03}W)",
            info.status.speed,
            average_speed,
            info.status.internet,
            td / 1000,
            ap / 1000,
            ap as f64 / td as f64 * 100.0,
            (td - ap) / 1000,
            (td - ap) as f64 / td as f64 * 100.0,
            next_stop_dist / 1000,
            next_stop_name,
            info.status.latitude,
            info.status.longitude
        );

        let block = if self.selection == PanelSelection::StatusInformation {
            Block::bordered()
                .title("Statusinformation")
                .border_style(Color::Magenta)
        } else {
            Block::bordered().title("Statusinformation")
        };

        frame.render_widget(Paragraph::new(content).block(block), area);
    }

    fn draw_speed_graph(&self, frame: &mut Frame, area: Rect) {
        let block = if self.selection == PanelSelection::SpeedInformation {
            Block::bordered()
                .title("Geschwindigkeitsverlauf")
                .border_style(Color::Magenta)
        } else {
            Block::bordered().title("Geschwindigkeitsverlauf")
        };

        let canvas = Canvas::default()
            .block(block)
            .x_bounds([0.0, self.data.capacity() as f64])
            .y_bounds([0.0, 300.0])
            .paint(|ctx| {
                for (xc, (curr, next)) in self.data.iter().zip(self.data.iter().skip(1)).enumerate()
                {
                    ctx.draw(&widgets::canvas::Line {
                        x1: xc as f64,
                        y1: curr.status.speed,
                        x2: xc as f64 + 1.0,
                        y2: next.status.speed,
                        color: if curr.status.speed > next.status.speed {
                            Color::Red
                        } else {
                            Color::Green
                        },
                    });
                }
            });

        frame.render_widget(canvas, area);
    }

    fn draw_trip(&self, frame: &mut Frame, area: Rect) {
        let info = self.data.back().expect("Nothing to draw");

        let lphk = 5; // lines per kilometers (TODO calculate appropriate value)

        let height = (area.height - 2) as usize; // subtract 2 for border

        let data_when: DateTime<Local> =
            DateTime::from_timestamp(info.status.serverTime as i64 / 1000, 0)
                .unwrap()
                .into();
        let now = Local::now().time();
        let diff = now - data_when.time();

        let block = if self.selection == PanelSelection::TripInformation {
            Block::bordered()
                .title("Streckenverlauf")
                .border_style(Color::Magenta)
                .title_bottom(format!(
                    "[Zuletzt aktualisiert: {} (vor {} Sekunden)]",
                    data_when.format("%H:%M:%S"),
                    diff.num_seconds()
                ))
        } else {
            Block::bordered()
                .title("Streckenverlauf")
                .title_bottom(format!(
                    "[Zuletzt aktualisiert: {} (vor {} Sekunden)]",
                    data_when.format("%H:%M:%S"),
                    diff.num_seconds()
                ))
        };

        let area_inside = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width - 2,
            height: area.height - 2,
        };

        let layout_constraints = info.trip.trip.stops.iter().map(|stop| {
            Constraint::Ratio(
                stop.info.distanceFromStart as u32,
                info.trip.trip.totalDistance as u32,
            )
        });
        let layouts = Layout::new(Direction::Vertical, layout_constraints).split(area_inside);

        // draw line (will get overwritten later on) (TODO)
        // for i in 0..area.height {
        //     frame.render_widget(Paragraph::new(" |"), R);
        // }

        // draw stations
        for (i, stop) in info.trip.trip.stops.iter().enumerate() {
            let layout = layouts[i];

            let text = if let Some(sat) = stop.timetable.scheduledArrivalTime {
                let time: DateTime<Local> = DateTime::from_timestamp(sat as i64 / 1000, 0)
                    .unwrap()
                    .into();
                let now = Local::now().time();
                let aat = stop
                    .timetable
                    .actualArrivalTime
                    .expect("If there is a scheduled time there should also be an actual time");
                let delay = (aat as i64 - sat as i64) / 1000 / 60;

                if delay == 0 {
                    format!("{} ({})", stop.station.name.clone(), time.format("%H:%M"))
                } else {
                    format!(
                        "{} ({}; {}{})",
                        stop.station.name.clone(),
                        time.format("%H:%M"),
                        if delay < 0 { "-" } else { "+" },
                        delay
                    )
                }
            } else {
                format!("{} (-)", stop.station.name.clone())
            };

            if self.selected_station_detailed && i == self.selected_station {
                // detailed information (delay reasons, track, coordinates, distance...)
                let track = format!(
                    "Gleis {}{}",
                    stop.track.actual,
                    if stop.track.actual == stop.track.scheduled {
                        String::from("")
                    } else {
                        format!("(urspr. {})", stop.track.scheduled)
                    }
                );

                let additional = format!("{}\n{}\n", text, track);
                frame.render_widget(Paragraph::new(additional), layout);
            } else {
                frame.render_widget(Paragraph::new(text), layout);
            }
        }

        let next_eva = &info.trip.trip.stopInfo.scheduledNext;
        let next_stop = info
            .trip
            .trip
            .stops
            .iter()
            .filter(|&stop| stop.station.evaNr == *next_eva)
            .collect::<Vec<_>>();
        assert_eq!(next_stop.len(), 1);

        frame.render_widget(Paragraph::new("").block(block), area);
    }

    fn ui(&self, frame: &mut Frame) {
        let layout = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::default(),
            ],
        )
        .split(frame.size());

        let layout_1 = Layout::new(
            Direction::Horizontal,
            [Constraint::Min(50), Constraint::default()],
        )
        .split(layout[1]);

        self.draw_basic_info(frame, layout[0]);
        self.draw_status(frame, layout_1[0]);
        self.draw_speed_graph(frame, layout_1[1]);
        self.draw_trip(frame, layout[2]);
    }

    // update state (query API, move graphs, ...)
    fn tick(&mut self) {
        // let files = ApiPaths {
        //     status: PathBuf::from("sample/status.json"),
        //     trip: PathBuf::from("sample/trip.json"),
        // };

        // let info = Info::from_file(&files).unwrap();

        let endpoints = ApiEndpoints {
            status: String::from("https://iceportal.de/api1/rs/status"),
            trip: String::from("https://iceportal.de/api1/rs/tripInfo/trip"),
        };

        let info = Info::query(&endpoints).unwrap();

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

            if event::poll(Duration::from_secs(1))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == event::KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => {
                                return Ok(true);
                            }
                            KeyCode::Tab => {
                                self.selection.next();
                            }
                            KeyCode::BackTab => {
                                self.selection.prev();
                            }
                            KeyCode::Enter => {
                                if self.selection == PanelSelection::TripInformation {
                                    self.selected_station_detailed =
                                        !self.selected_station_detailed;
                                }
                            }
                            KeyCode::Char('k') => {
                                if let Some(info) = self.data.back() {
                                    if self.selected_station_detailed {
                                        self.selected_station = (self.selected_station + 1)
                                            .min(info.trip.trip.stops.len() - 1);
                                    }
                                }
                            }
                            KeyCode::Char('j') => {
                                if let Some(info) = self.data.back() {
                                    if self.selected_station_detailed {
                                        self.selected_station = self
                                            .selected_station
                                            .checked_sub(1)
                                            .unwrap_or(self.selected_station);
                                    }
                                }
                            }
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
