use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn main() -> std::io::Result<()> {
    let socket_path = "/run/contextd/public/contextd.socket";
    
    println!("Connecting to {}...", socket_path);
    
    // Connect to the Unix socket
    let mut stream = match UnixStream::connect(socket_path) {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("Failed to connect to contextd socket: {}", e);
            eprintln!("Is the contextd daemon running?");
            std::process::exit(1);
        }
    };
    
    // Format the varlink request (JSON object)
    let request = serde_json::json!({
        "method": "com.performativenonsense.contextd.GetActiveGame",
        "parameters": {}
    });
    
    // Varlink requires requests to be terminated by a null byte
    let mut message = serde_json::to_vec(&request).unwrap();
    message.push(b'\0');
    
    // Send the request
    stream.write_all(&message)?;
    
    // Read the response up to the null byte
    let mut buffer = Vec::new();
    let mut buf = [0; 1024];
    loop {
        let bytes_read = stream.read(&mut buf)?;
        if bytes_read == 0 {
            break;
        }
        for &b in &buf[..bytes_read] {
            if b == b'\0' {
                // Parse and print the response
                if let Ok(response) = serde_json::from_slice::<serde_json::Value>(&buffer) {
                    println!("Response:\n{:#?}", response);
                }
                return Ok(());
            }
            buffer.push(b);
        }
    }
    
    Ok(())
}
