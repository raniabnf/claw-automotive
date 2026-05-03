use automotive_host::host::{AutomotiveHostAgent, HostConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = AutomotiveHostAgent::new(HostConfig::default());
    let args: Vec<String> = std::env::args().skip(1).collect();

    let output = match args.first().map(String::as_str) {
        Some("--print-bridge") => serde_json::to_string_pretty(&host.bridge)?,
        Some("--print-adapters") => serde_json::to_string_pretty(&host.adapters)?,
        _ => serde_json::to_string_pretty(&host)?,
    };

    println!("{output}");
    Ok(())
}
