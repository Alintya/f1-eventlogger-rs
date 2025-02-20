use crate::session::SessionState;
use clap::Parser;

use f1_telemetry::packet::Packet;
use f1_telemetry::Stream;

mod session;

#[derive(Parser)]
#[command(author, version, about, propagate_version = true)]
struct AppArgs {
    /// Host to bind on for the UDP packet listener
    #[clap(long, default_value = "127.0.0.1", env)]
    listener_host: String,

    /// Port to bind on for the UDP packet listener
    #[clap(long, default_value = "20777", env)]
    listener_port: u16,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = AppArgs::parse();
    let telemetry_addr = format!("{}:{}", args.listener_host, args.listener_port);
    let packet_stream = Stream::new(&telemetry_addr).await?;

    println!("Collecting telemetry from: {}", telemetry_addr);

    let mut session_state = SessionState::new();

    loop {
        match packet_stream.next().await {
            Ok(p) => match p {
                Packet::Session(sp) => {
                    session_state.update_session(sp)?;
                },
                Packet::Participants(pp) => {
                    session_state.cars = pp.participants;
                },
                Packet::Event(event) => {
                    if session_state.is_logging_enabled() {
                        session_state.handle_overtake(&event)?;
                    }
                },
                Packet::CarTelemetry(ctp) => {
                    session_state.update_car_speeds(&ctp.car_telemetry_data);
                },
                Packet::CarStatus(cs) => {
                    session_state.car_status = cs.car_status_data;
                },
                Packet::LapData(lp) => {
                    session_state.lap_data = lp.lap_data;
                },
                Packet::FinalClassification(fc) => {
                    session_state.write_final_classification(fc)?;
                },
                _ => {},
            },
            Err(err) => {
                println!("{:?}", err);
            },
        }
    }
}
