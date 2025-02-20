use f1_telemetry::packet::car_status::CarStatusData;
use f1_telemetry::packet::car_telemetry::CarTelemetryData;
use f1_telemetry::packet::event::{Event, Overtake, PacketEventData};
use f1_telemetry::packet::final_classification::PacketFinalClassificationData;
use f1_telemetry::packet::lap::LapData;
use f1_telemetry::packet::participants::ParticipantData;
use f1_telemetry::packet::session::{PacketSessionData, RuleSet};
use std::{fs, io, path};

#[derive(Debug, Clone, PartialEq, Eq)]
struct OvertakeEventLog {
    overtaker_name: String,
    overtaker_team: String,
    overtaker_speed: u16,
    overtaker_tyre_compound: String,
    overtaker_tyre_age: u8,
    overtakee_name: String,
    overtakee_team: String,
    overtakee_speed: u16,
    overtakee_tyre_compound: String,
    overtakee_tyre_age: u8,
    for_pos: u8,
    lap: u8,
    track_position: u16,
    time_secs: u32,
}

const OVERTAKE_CSV_HEADERS: [&str; 14] = [
    "Overtaker",
    "Overtaker Team",
    "Overtaker Speed",
    "Overtaker Tyre Compound",
    "Overtaker Tyre Age",
    "Overtakee",
    "Overtakee Team",
    "Overtakee Speed",
    "Overtakee Tyre Compound",
    "Overtakee Tyre Age",
    "For Position",
    "Lap",
    "Track Position",
    "Sessiontime [ms]",
];

const CLASSIFICATION_CSV_HEADERS: [&str; 11] = [
    "Position",
    "Driver",
    "Team",
    "Grid Position",
    "Fastest Lap Time [ms]",
    "Finish Time [ms]",
    "Laps",
    "Pitstops",
    "Penalties",
    "Penalty Time [s]",
    "Status",
];

pub(crate) struct SessionState {
    session_info: Option<PacketSessionData>,
    session_uid: u64,
    pub(crate) cars: Vec<ParticipantData>,
    pub(crate) car_status: Vec<CarStatusData>,
    pub(crate) lap_data: Vec<LapData>,

    car_speeds: Vec<u16>,
    csv_writer: Option<csv::Writer<fs::File>>,
}

impl SessionState {
    pub(crate) fn new() -> Self {
        Self {
            session_info: None,
            session_uid: u64::MIN,
            cars: Vec::with_capacity(22), // Pre-allocate for max F1 grid size
            car_status: Vec::with_capacity(22),
            lap_data: Vec::with_capacity(22),
            car_speeds: Vec::with_capacity(22),
            csv_writer: None,
        }
    }

    pub(crate) fn is_logging_enabled(&self) -> bool {
        self.csv_writer.is_some()
    }

    pub(crate) fn update_session(&mut self, session_data: PacketSessionData) -> io::Result<()> {
        // Only flush and update if session has changed
        if self.session_uid != session_data.header.session_uid {
            if let Some(writer) = self.csv_writer.as_mut() {
                writer.flush()?;
            }
            self.session_uid = session_data.header.session_uid;

            self.csv_writer = if session_data.rule_set == Some(RuleSet::Race) {
                Some(self.create_new_csv_writer(&session_data, "Events", &OVERTAKE_CSV_HEADERS)?)
            } else {
                println!("Not a race or sprint session - skipping event logging");
                None
            };
        }

        self.session_info = Some(session_data);

        Ok(())
    }

    pub(crate) fn handle_overtake(&mut self, event: &PacketEventData) -> Result<(), Box<dyn std::error::Error>> {
        // Early return if no CSV writer or no car data
        if self.csv_writer.is_none() || self.cars.is_empty() {
            return Ok(());
        }

        if let Event::Overtake(ot) = event.event {
            let overtake_event = self.create_overtake_event(&ot, event.header.session_time)?;
            self.write_overtake_event(&overtake_event)?;
        }

        Ok(())
    }

    pub(crate) fn write_final_classification(
        &self,
        fc: PacketFinalClassificationData,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session_info = self
            .session_info
            .as_ref()
            .ok_or_else(|| Box::<dyn std::error::Error>::from("No session info available"))?;

        let mut writer = self.create_new_csv_writer(session_info, "Results", &CLASSIFICATION_CSV_HEADERS)?;

        for (i, result) in fc.final_classifications.iter().enumerate().take(fc.num_cars as usize) {
            let car = self.cars.get(i).ok_or_else(|| Box::<dyn std::error::Error>::from("Car data not found"))?;

            writer.write_record(&[
                result.position.to_string(),
                car.name.clone(),
                format!("{} ({})", car.team.name(), car.race_number),
                result.grid_position.to_string(),
                result.best_lap_time.to_string(),
                result.total_race_time.to_string(),
                result.num_laps.to_string(),
                result.num_pit_stops.to_string(),
                result.num_penalties.to_string(),
                result.penalties_time.to_string(),
                format!("{:?}", result.result_status),
            ])?;
        }

        writer.flush()?;
        Ok(())
    }

    pub(crate) fn update_car_speeds(&mut self, telemetry: &[CarTelemetryData]) {
        self.car_speeds.clear();
        self.car_speeds.extend(telemetry.iter().map(|car| car.speed));
    }

    fn create_overtake_event(
        &self,
        ot: &Overtake,
        session_time: u32,
    ) -> Result<OvertakeEventLog, Box<dyn std::error::Error>> {
        let get_car = |idx: u8| -> Result<&ParticipantData, Box<dyn std::error::Error>> {
            self.cars.get(idx as usize).ok_or_else(|| Box::from("Car data not found"))
        };
        let get_status = |idx: u8| -> Result<&CarStatusData, Box<dyn std::error::Error>> {
            self.car_status.get(idx as usize).ok_or_else(|| Box::from("Car status not found"))
        };
        let get_speed = |idx: u8| -> u16 { self.car_speeds.get(idx as usize).copied().unwrap_or(0) };

        let overtaker = get_car(ot.overtaking_vehicle_idx)?;
        let overtaker_status = get_status(ot.overtaking_vehicle_idx)?;
        let overtakee = get_car(ot.being_overtaken_vehicle_idx)?;
        let overtakee_status = get_status(ot.being_overtaken_vehicle_idx)?;
        let lap = self
            .lap_data
            .get(ot.being_overtaken_vehicle_idx as usize)
            .ok_or_else(|| Box::<dyn std::error::Error>::from("Lap data not found"))?;

        Ok(OvertakeEventLog {
            overtaker_name: overtaker.name.clone(),
            overtaker_team: format!("{} ({})", overtaker.team.name(), overtaker.race_number),
            overtaker_speed: get_speed(ot.overtaking_vehicle_idx),
            overtaker_tyre_compound: overtaker_status.visual_tyre_compound.name().to_string(),
            overtaker_tyre_age: overtaker_status.tyre_age_laps.unwrap_or(u8::MAX),
            overtakee_name: overtakee.name.clone(),
            overtakee_team: format!("{} ({})", overtakee.team.name(), overtakee.race_number),
            overtakee_speed: get_speed(ot.being_overtaken_vehicle_idx),
            overtakee_tyre_compound: overtakee_status.visual_tyre_compound.name().to_string(),
            overtakee_tyre_age: overtakee_status.tyre_age_laps.unwrap_or(u8::MAX),
            for_pos: lap.car_position,
            lap: lap.current_lap_num,
            track_position: lap.lap_distance as u16,
            time_secs: session_time,
        })
    }

    fn create_new_csv_writer(
        &self,
        session_data: &PacketSessionData,
        event_type: &str,
        headers: &[&str],
    ) -> io::Result<csv::Writer<fs::File>> {
        let filename = path::PathBuf::from(format!(
            "{} {} {}_{}.csv",
            session_data.track.name(),
            session_data.session_type.name(),
            event_type,
            session_data.header.session_uid,
        ));
        println!("Writing {} to {:?}", event_type.to_lowercase(), &filename);

        let mut writer = csv::Writer::from_path(&filename)?;
        writer.write_record(headers)?;

        Ok(writer)
    }

    fn write_overtake_event(&mut self, event: &OvertakeEventLog) -> io::Result<()> {
        if let Some(writer) = self.csv_writer.as_mut() {
            writer.write_record([
                &event.overtaker_name,
                &event.overtaker_team,
                &event.overtaker_speed.to_string(),
                &event.overtaker_tyre_compound,
                &event.overtaker_tyre_age.to_string(),
                &event.overtakee_name,
                &event.overtakee_team,
                &event.overtakee_speed.to_string(),
                &event.overtakee_tyre_compound,
                &event.overtakee_tyre_age.to_string(),
                &event.for_pos.to_string(),
                &event.lap.to_string(),
                &event.track_position.to_string(),
                &event.time_secs.to_string(),
            ])?;
            writer.flush()?;
        }
        Ok(())
    }
}
