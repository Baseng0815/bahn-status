use std::{io, path::PathBuf};

use ratatui::{
    crossterm::event::{self, Event, KeyCode}, layout::{Constraint, Direction, Layout, Rect}, style::Color, text::{Line, Span, Text}, widgets::{self, canvas::{Canvas, Map, MapResolution, Shape}, Block, Paragraph}, Frame
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

fn draw_basic_info(frame: &mut Frame, area: Rect, info: &Info) {
    let content = format!("\
Schienenfahrzeugtyp:           {}
Schienenfahrzeugbezeichnung:   {}
Sozioökonomisches Milieu:      {}
Streckenführung:               von {} nach {}
", info.status.trainType, info.status.tzn, info.status.wagonClass,
info.trip.trip.stops.first().expect("Everything has to start somewhere").station.name,
info.trip.trip.stops.last().expect("Everything has to end somewhere").station.name);

    let block = Block::bordered().title("Grundlegende Informationen");
    frame.render_widget(Paragraph::new(content).block(block), area);
}

fn draw_status(frame: &mut Frame, area: Rect, info: &Info) {
    let ap = info.trip.trip.actualPosition;
    let td = info.trip.trip.totalDistance;

    let content = format!("\
Aktuelle Geschwindigkeit:      {}km/h
Internetzwerkverbindungsgüte:  {}
Gesamte Streckenlänge:         {}km
Davon bereits zurückgelegt:    {}km ({:.2}%)
Verbleibend (nach Adam Riese): {}km ({:.2}%)
Entfernung zum nächsten Halt:  {}km ({})
Aktuelle geographische Lage:   ({:.03}N, {:.03}W)",
info.status.speed, info.status.internet, td / 1000, ap / 1000, ap as f64 / td as f64 * 100.0,
(td - ap) / 1000, (td - ap) as f64 / td as f64 * 100.0, 0, "NEXT STOP", info.status.latitude, info.status.longitude);

    let block = Block::bordered().title("Statusinformation");
    frame.render_widget(Paragraph::new(content).block(block), area);
}

// struct TripShape {
//     stations: Vec<Station>
// }

// impl Shape for TripShape {
//     fn draw(&self, painter: &mut ratatui::widgets::canvas::Painter) {
//         painter.li
//     }
// }

fn draw_trip(frame: &mut Frame, area: Rect, info: &Info) {
    let lphk = 5; // lines per kilometers (TODO calculate appropriate value)

    let height = (area.height - 2) as usize; // subtract 2 for border

    let (mut miny, mut maxy, mut minx, mut maxx) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for stop in &info.trip.trip.stops {
        miny = stop.station.geocoordinates.latitude.min(miny);
        maxy = stop.station.geocoordinates.latitude.max(maxy);
        minx = stop.station.geocoordinates.longitude.min(minx);
        maxx = stop.station.geocoordinates.longitude.max(maxx);
    }

    let block = Block::bordered().title("Streckenverlauf");
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

                ctx.print(curr.station.geocoordinates.longitude, curr.station.geocoordinates.latitude, Line::from(curr.station.name.clone()));
            }
        });

    frame.render_widget(canvas, area);
}

pub fn ui(frame: &mut Frame) {
    let layout = Layout::new(Direction::Vertical, [ Constraint::Length(6), Constraint::Length(9), Constraint::default() ])
        .split(frame.size());

    // TODO move into main and somehow pass to here (also query regularly)
    let files = ApiPaths {
        status: PathBuf::from("sample/status.json"),
        trip: PathBuf::from("sample/trip.json"),
    };

    let info = Info::from_file(&files).unwrap();

    draw_basic_info(frame, layout[0], &info);
    draw_status(frame, layout[1], &info);
    draw_trip(frame, layout[2], &info);
}

pub fn handle_events() -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(true);
            }
        }
    }

    Ok(false)
}
