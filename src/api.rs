// API access and data structures

// Status

use std::{
    error::Error,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use rand::Rng;
use serde::Deserialize;

#[derive(Default, Deserialize, Debug)]
pub struct ApiEndpoints {
    pub status: String,
    pub trip: String,
}

#[derive(Default, Deserialize, Debug)]
pub struct ApiPaths {
    pub status: PathBuf,
    pub trip: PathBuf,
}

#[derive(Default, Deserialize, Debug)]
pub struct Connectivity {
    currentState: String,
    nextState: Option<String>,
    remainingTimeSeconds: Option<u64>,
}

#[derive(Default, Deserialize, Debug)]
pub struct StatusInfo {
    pub connection: bool, // no idea what is is
    pub serviceLevel: String,
    pub gpsStatus: String,
    pub internet: String,
    pub latitude: f64,
    pub longitude: f64,
    pub tileY: i64,
    pub tileX: i64,
    pub series: String, // TODO parse in a better way (I'm not a train nerd so w/e)
    pub serverTime: u64,
    pub speed: f64,
    pub trainType: String,
    pub tzn: String, // train number
    pub wagonClass: String,
    connectivity: Connectivity,
    pub bapInstalled: bool, // bap = bahn-api ?
}

// Trip

#[derive(Default, Deserialize, Debug)]
pub struct TripStopInfo {
    pub scheduledNext: String,
    pub actualNext: String,
    pub actualLast: String,
    pub actualLastStarted: String,
    pub finalStationName: String,
    pub finalStationEvaNr: String,
}

#[derive(Default, Deserialize, Debug)]
pub struct GeoCoordinates {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Default, Deserialize, Debug)]
pub struct Station {
    pub evaNr: String,
    pub name: String,
    pub code: Option<String>,
    pub geocoordinates: GeoCoordinates,
}

#[derive(Default, Deserialize, Debug)]
pub struct Timetable {
    pub scheduledArrivalTime: Option<u64>, // option since no arrival at first station
    pub actualArrivalTime: Option<u64>,
    pub showActualArrivalTime: Option<bool>,
    pub arrivalDelay: Option<String>,
    pub scheduledDepartureTime: Option<u64>, // option since no departure from last station
    pub actualDepartureTime: Option<u64>,
    pub showActualDepartureTime: Option<bool>,
    pub departureDelay: Option<String>,
}

#[derive(Default, Deserialize, Debug)]
pub struct Track {
    pub scheduled: String,
    pub actual: String,
}

#[derive(Default, Deserialize, Debug)]
pub struct StopInfo {
    pub status: u64,
    pub passed: bool,
    pub positionStatus: String,
    pub distance: u64,
    pub distanceFromStart: u64,
}

#[derive(Default, Deserialize, Debug)]
pub struct DelayReason {
    // TODO can't fill this out yet (the train is actually on time)
}

#[derive(Default, Deserialize, Debug)]
pub struct Stop {
    pub station: Station,
    pub timetable: Timetable,
    pub track: Track,
    pub info: StopInfo,
    pub delay_reasons: Option<Vec<DelayReason>>,
}

#[derive(Default, Deserialize, Debug)]
pub struct Connection {
    pub trainType: Option<String>,
    pub vzn: Option<String>,
    pub trainNumber: Option<String>,
    pub station: Option<Station>,
    pub timetable: Option<Timetable>,
    pub track: Option<Track>,
    pub info: Option<TripStopInfo>,
    pub stops: Option<Vec<Stop>>,
    pub conflict: String,
}

#[derive(Default, Deserialize, Debug)]
pub struct Trip {
    pub tripDate: String,
    pub trainType: String,
    pub vzn: String, // train identifier
    pub actualPosition: u64,
    pub distanceFromLastStop: u64,
    pub totalDistance: u64,
    pub stopInfo: TripStopInfo,
    pub stops: Vec<Stop>,
}

#[derive(Default, Deserialize, Debug)]
pub struct TripInfo {
    pub trip: Trip,
    pub connection: Connection,
    pub active: Option<bool>,
}

#[derive(Default, Deserialize, Debug)]
pub struct Info {
    pub status: StatusInfo,
    pub trip: TripInfo,
}

impl StatusInfo {
    pub fn query(endpoint: &str) -> Result<StatusInfo, reqwest::Error> {
        let client = reqwest::blocking::Client::new();

        let response = client
            .get(endpoint)
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
            )
            .send()?;
        let deserialized = response.json()?;
        Ok(deserialized)
    }

    pub fn from_file(path: &Path) -> Result<StatusInfo, Box<dyn Error>> {
        let content = fs::read_to_string(path)?;
        let mut status: StatusInfo = serde_json::from_str(&content)?;
        status.speed = rand::thread_rng().gen_range(0.0..300.0);
        Ok(status)
    }
}

impl TripInfo {
    pub fn query(endpoint: &str) -> Result<TripInfo, reqwest::Error> {
        let client = reqwest::blocking::Client::new();

        let response = client
            .get(endpoint)
            .header(
                "User-Agent",
                "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
            )
            .send()?;
        let deserialized = response.json()?;
        Ok(deserialized)
    }

    pub fn from_file(path: &Path) -> Result<TripInfo, Box<dyn Error>> {
        let content = fs::read_to_string(path)?;
        let trip: TripInfo = serde_json::from_str(&content)?;
        Ok(trip)
    }
}

impl Info {
    pub fn query(endpoints: &ApiEndpoints) -> Result<Info, reqwest::Error> {
        let status = StatusInfo::query(&endpoints.status)?;
        let trip = TripInfo::query(&endpoints.trip)?;

        Ok(Info { status, trip })
    }

    pub fn from_file(paths: &ApiPaths) -> Result<Info, Box<dyn Error>> {
        let status = StatusInfo::from_file(&paths.status)?;
        let trip = TripInfo::from_file(&paths.trip)?;

        Ok(Info { status, trip })
    }
}
