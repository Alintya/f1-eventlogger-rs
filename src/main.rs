use clap::Parser;
use f1_telemetry::packet::event::Event;
use f1_telemetry::packet::Packet;
use f1_telemetry::Stream;

#[derive(Parser)]
#[command(author, version, about, long_about = None, propagate_version = true)]
struct AppArgs {
    /// Host to bind on for the UDP packet listener
    #[clap(long, default_value = "127.0.0.1", env)]
    listener_host: String,

    /// Port to bind on for the UDP packet listener
    #[clap(long, default_value = "20777", env)]
    listener_port: u16,
}

struct SessionState {
    session_info: f1_telemetry::packet::session::PacketSessionData,
    cars: Vec<f1_telemetry::packet::participants::ParticipantData>,
    car_status: Vec<f1_telemetry::packet::car_status::CarStatusData>,
    lap_data: Vec<f1_telemetry::packet::lap::LapData>,
    car_speeds: Vec<u16>,
    csv_writer: csv::Writer<std::fs::File>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = AppArgs::parse();

    let telemetry_addr = format!("{}:{}", args.listener_host, args.listener_port);
    let packet_stream =
        Stream::new(&telemetry_addr).await.expect("Failed to create packet stream.");

    println!("Collecting telemetry from: {}", telemetry_addr);

    let mut current_session_info: Option<f1_telemetry::packet::session::PacketSessionData> = None;
    let mut last_session_uid = u64::MIN;

    let mut car_speeds = Vec::new();

    let mut cars = Vec::new();
    let mut cars_status: Vec<f1_telemetry::packet::car_status::CarStatusData> = Vec::new();
    let mut lap_data: Vec<f1_telemetry::packet::lap::LapData> = Vec::new();

    let mut csv_writer: Option<csv::Writer<std::fs::File>> = None;

    loop {
        match packet_stream.next().await {
            Ok(p) => match p {
                Packet::Session(sp) => {
                    current_session_info = Some(sp.clone());

                    if let Some(w) = csv_writer.as_mut() {
                        // TODO session or track changed
                        if last_session_uid != sp.header.session_uid {
                            //sp.session_type
                            w.flush()?;
                            csv_writer = None;
                        }
                    }
                    if csv_writer.is_none() {
                        last_session_uid = sp.header.session_uid;

                        let filename = format!(
                            "{} {} {}_{}.csv",
                            current_session_info.as_ref().unwrap().track.name(),
                            current_session_info.as_ref().unwrap().session_type.name(),
                            "Events",
                            sp.header.session_uid,
                        );

                        println!("Writing events to {}", &filename);

                        csv_writer = Some(
                            csv::Writer::from_path(&filename).expect("Failed to create CSV writer"),
                        );
                        if let Err(e) = csv_writer.as_mut().unwrap().write_record([
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
                            "Sessiontime",
                        ]) {
                            eprintln!("Error writing header: {:?}", e);
                        }
                    }
                },
                Packet::Participants(pp) => {
                    cars = pp.participants;
                },
                Packet::Event(event) => {
                    if cars.is_empty() {
                        continue;
                    }

                    if let Event::Overtake(ot) = event.event {
                        let overtaker = cars.get(ot.overtaking_vehicle_idx as usize).unwrap();
                        let overtaker_car_status =
                            cars_status.get(ot.overtaking_vehicle_idx as usize).unwrap();
                        let being_overtaken =
                            cars.get(ot.being_overtaken_vehicle_idx as usize).unwrap();
                        let being_overtaken_car_status =
                            cars_status.get(ot.being_overtaken_vehicle_idx as usize).unwrap();

                        let lap = match lap_data.get(ot.being_overtaken_vehicle_idx as usize) {
                            Some(lap) => lap,
                            None => continue,
                        };

                        let ot_event = OvertakeEventLog {
                            overtaker_name: overtaker.name.clone(),
                            overtaker_team: overtaker.team.name().to_string(),
                            overtaker_speed: *car_speeds
                                .get(ot.overtaking_vehicle_idx as usize)
                                .unwrap_or(&0),
                            overtaker_tyre_compound: overtaker_car_status
                                .visual_tyre_compound
                                .name()
                                .to_string(),
                            overtaker_tyre_age: overtaker_car_status
                                .tyre_age_laps
                                .unwrap_or(u8::MAX),
                            being_overtaken_name: being_overtaken.name.clone(),
                            being_overtaken_team: being_overtaken.team.name().to_string(),
                            being_overtaken_speed: *car_speeds
                                .get(ot.being_overtaken_vehicle_idx as usize)
                                .unwrap_or(&0),
                            being_overtaken_tyre_compound: being_overtaken_car_status
                                .visual_tyre_compound
                                .name()
                                .to_string(),
                            being_overtaken_tyre_age: being_overtaken_car_status
                                .tyre_age_laps
                                .unwrap_or(u8::MAX),
                            for_pos: lap.car_position,
                            lap: lap.current_lap_num,
                            track_position: lap.lap_distance as u16,
                            time_secs: event.header.session_time / 1000,
                        };

                        //println!("{:?}", ot_event);
                        if let Some(w) = csv_writer.as_mut() {
                            w.write_record(&[
                                ot_event.overtaker_name,
                                ot_event.overtaker_team,
                                ot_event.overtaker_speed.to_string(),
                                ot_event.overtaker_tyre_compound,
                                ot_event.overtaker_tyre_age.to_string(),
                                ot_event.being_overtaken_name,
                                ot_event.being_overtaken_team,
                                ot_event.being_overtaken_speed.to_string(),
                                ot_event.being_overtaken_tyre_compound,
                                ot_event.being_overtaken_tyre_age.to_string(),
                                ot_event.for_pos.to_string(),
                                ot_event.lap.to_string(),
                                ot_event.track_position.to_string(),
                                ot_event.time_secs.to_string(),
                            ])
                            .unwrap();

                            w.flush()?;
                        }
                    }
                },
                Packet::CarTelemetry(ctp) => {
                    for (i, car) in ctp.car_telemetry_data.iter().enumerate() {
                        match car_speeds.get_mut(i) {
                            Some(v) => *v = car.speed,
                            None => car_speeds.push(car.speed),
                        }
                    }
                },
                Packet::CarStatus(cs) => {
                    cars_status.clone_from(&cs.car_status_data);
                },
                Packet::LapData(lp) => {
                    lap_data.clone_from(&lp.lap_data);
                },
                Packet::FinalClassification(fc) => {
                    let filename = format!(
                        "{} {} {}_{}.csv",
                        current_session_info.as_ref().unwrap().track.name(),
                        current_session_info.as_ref().unwrap().session_type.name(),
                        "Results",
                        fc.header.session_uid,
                    );

                    println!("Writing final classification to {}", &filename);

                    let mut writer = csv::Writer::from_path(&filename)?;

                    writer.write_record([
                        "Position",
                        "Driver",
                        "Team",
                        "Grid Position",
                        "Fastest Lap Time [ms]",
                        "Total Time [ms]",
                    ])?;

                    for (i, result) in
                        fc.final_classifications.iter().enumerate().take(fc.num_cars as usize)
                    {
                        let car = cars.get(i).unwrap();

                        writer.write_record(&[
                            result.position.to_string(),
                            car.name.clone(),
                            car.team.name().to_string(),
                            result.grid_position.to_string(),
                            result.best_lap_time.to_string(),
                            result.total_race_time.to_string(),
                        ])?;
                    }
                },
                _ => {},
            },
            Err(e) => {
                println!("Packet error!: {:?}", e);
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OvertakeEventLog {
    overtaker_name: String,
    overtaker_team: String,
    overtaker_speed: u16,
    overtaker_tyre_compound: String,
    overtaker_tyre_age: u8,
    being_overtaken_name: String,
    being_overtaken_team: String,
    being_overtaken_speed: u16,
    being_overtaken_tyre_compound: String,
    being_overtaken_tyre_age: u8,
    for_pos: u8,
    lap: u8,
    track_position: u16,
    time_secs: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PenaltyEventLog {
    driver: String,
    team: String,
    lap: u8,
    track_position: u16,
    time_secs: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Car {
    driver_name: String,
    team_name: String,
}
