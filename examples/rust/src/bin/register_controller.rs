use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process;

/// Minimal representation of the Controller type for the example
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Controller {
    name: String,
    version: Option<String>,
    pid: i64,
    description: Option<String>,
    website: Option<String>,
    capabilities: Vec<String>,
    interested_devices: Vec<String>,
}

#[derive(Serialize)]
struct RegisterArgs {
    controller: Controller,
}

#[derive(Serialize)]
struct UnregisterArgs {
    pid: i64,
}

#[derive(Serialize)]
struct VarlinkRequest<T> {
    method: String,
    parameters: T,
}

fn main() -> Result<()> {
    let socket_path = "/run/contextd/public/contextd.socket";
    let mut stream = UnixStream::connect(socket_path)?;

    let pid = process::id() as i64;
    let hint = Controller {
        name: "Rust Example Agent".to_string(),
        version: Some("0.1.0".to_string()),
        pid,
        description: Some("Cooperative hardware manager example in Rust".to_string()),
        website: None,
        capabilities: vec!["rust".to_string(), "demo".to_string()],
        interested_devices: vec!["/dev/hidraw0".to_string()],
    };

    println!("Registering hint for PID {}...", pid);
    
    // Register
    let req = VarlinkRequest {
        method: "com.performativenonsense.contextd.RegisterController".to_string(),
        parameters: RegisterArgs { controller: hint },
    };
    
    let mut payload = serde_json::to_vec(&req)?;
    payload.push(0); // Varlink uses null-terminator for messages
    stream.write_all(&payload)?;

    // Read the response (Varlink requires reading the response to ensure the call completed)
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;
    println!("Response: {}", String::from_utf8_lossy(&buf[..n]).trim_end_matches('\0'));

    println!("\nRegistered successfully. You can verify this by running:");
    println!("  varlinkctl call unix:{} com.performativenonsense.contextd.ListControllers", socket_path);
    
    println!("\nPress Enter to unregister and exit...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Unregister cleanly
    let mut stream = UnixStream::connect(socket_path)?;
    let req = VarlinkRequest {
        method: "com.performativenonsense.contextd.UnregisterController".to_string(),
        parameters: UnregisterArgs { pid },
    };
    let mut payload = serde_json::to_vec(&req)?;
    payload.push(0);
    stream.write_all(&payload)?;
    
    println!("Unregistered. Goodbye!");

    Ok(())
}
