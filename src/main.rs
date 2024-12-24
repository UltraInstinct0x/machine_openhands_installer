use std::process::Command;
use warp::Filter;
use tokio::sync::mpsc;
use tokio::time::Duration;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use serde_json::json;
use warp::{Reply, Rejection};
use std::convert::Infallible;


const FULL_COMMAND: &str = "
docker pull docker.all-hands.dev/all-hands-ai/runtime:0.16-nikolaik && \
docker run -d --rm --pull=always \
    -e SANDBOX_RUNTIME_CONTAINER_IMAGE=docker.all-hands.dev/all-hands-ai/runtime:0.16-nikolaik \
    -e LOG_ALL_EVENTS=true \
    -v /var/run/docker.sock:/var/run/docker.sock \
    -v $HOME/.openhands:/home/openhands/.openhands \
    -p 3000:3000 \
    --add-host host.docker.internal:host-gateway \
    --name openhands-app \
    docker.all-hands.dev/all-hands-ai/openhands:0.16
";

struct OpenHandsSocket {
    tx: mpsc::Sender<String>,
}

impl OpenHandsSocket {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Wait for the container to be fully operational like waiting for your GPU drivers to update
        println!("Waiting for OpenHands container to be fully operational... 🚀");
        Self::wait_for_server().await?;

        // First get that Engine.IO session ID like collecting rare Pokemon
        let client = reqwest::Client::new();
        let handshake_url = "http://localhost:3000/socket.io/?EIO=4&transport=polling";
        
        println!("Initiating handshake with the mothership... 👽");
        let response = client.get(handshake_url).send().await?;
        let text = response.text().await?;
        
        // Extract that session ID from the response like mining for diamonds
        let sid = text
            .split("\"sid\":\"")
            .nth(1)
            .and_then(|s| s.split("\"").next())
            .ok_or("Failed to extract session ID")?;

        println!("Got session ID: {}... vibing...", &sid[..8]);

        // Now construct our WebSocket URL with that precious session ID
        let auth_json = serde_json::json!({"github_token": null}).to_string();
        let encoded_auth = urlencoding::encode(&auth_json);
        let ws_url = format!(
            "ws://localhost:3000/socket.io/?EIO=4&transport=websocket&sid={}&auth={}",
            sid,
            encoded_auth
        );

        // Attempt connection with retry logic because we're persistent like that
        let mut attempts = 0;
        let max_attempts = 5;
        let mut last_error = None;

        while attempts < max_attempts {
            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    println!("WebSocket connection established successfully! 🎉");
                    let (mut write, mut read) = ws_stream.split();
                    let (tx, mut rx) = mpsc::channel(32);

                    // Send the upgrade packet like we're evolving a Pokémon
                    write.send("2probe".into()).await?;

                    // Rest of the handler setup...
                    tokio::spawn(async move {
                        tokio::spawn(async move {
                            while let Some(msg) = read.next().await {
                                match msg {
                                    Ok(msg) => println!("Server spitting facts: {:?}", msg),
                                    Err(e) => println!("Socket got ratioed: {}", e),
                                }
                            }
                        });

                        while let Some(cmd) = rx.recv().await {
                            println!("Sending command to OpenHands: {}", cmd);
                            let event = format!("42[\"message\",{}]", json!({
                                "action": "RUN",
                                "args": {
                                    "command": cmd,
                                    "hidden": false
                                }
                            }));
                            
                            if let Err(e) = write.send(event.into()).await {
                                println!("Failed to send command: {}", e);
                            }
                        }
                    });

                    return Ok(Self { tx });
                }
                Err(e) => {
                    last_error = Some(e);
                    attempts += 1;
                    println!("Connection attempt {} failed, retrying in 2s... 😤", attempts);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Err(format!("Failed after {} attempts. Last error: {:?}", max_attempts, last_error).into())
    }

    async fn wait_for_server() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let mut attempts = 0;
        let max_attempts = 15;  // 30 seconds total with 2s delay

        while attempts < max_attempts {
            match client.get("http://localhost:3000").send().await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    attempts += 1;
                    println!("Server not ready, attempt {}/{}... 😴", attempts, max_attempts);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Err("Server failed to start in time, sadge".into())
    }

    async fn send_command(&self, command: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.tx.send(command.to_string()).await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    if !is_docker_installed() {
        println!("Docker not detected. Installing...");
        if let Err(e) = install_docker() {
            eprintln!("Failed to install Docker: {}", e);
            return;
        }
        println!("Docker installed successfully.");
    } else {
        println!("Docker is already installed.");
    }

    println!("Checking if the container image exists...");
    if is_container_image_present() {
        println!("Container image found. Running container...");
        if let Err(e) = run_container().await {
            eprintln!("Failed to run container: {}", e);
        }
    } else {
        println!("Container image not found. Executing FULL_COMMAND...");
        if let Err(e) = execute_full_command().await {
            eprintln!("Failed to execute FULL_COMMAND: {}", e);
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    let socket = match OpenHandsSocket::new().await {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("Failed to connect to OpenHands: {}", e);
            return;
        }
    };

    let socket = std::sync::Arc::new(socket);
    let socket_clone = socket.clone();
    let routes = warp::post()
        .and(warp::path("run"))
        .and(warp::body::json())
        .and_then(move |cmd: serde_json::Value| {
            let socket = socket_clone.clone();
            let command = cmd["command"].as_str().unwrap_or("").to_string();
            
            async move {
                let result = socket.send_command(&command).await;
                let reply = match result {
                    Ok(_) => json!({
                        "status": "sent",
                        "message": "Command forwarded to GUI"
                    }),
                    Err(e) => json!({
                        "status": "error",
                        "message": format!("Failed to send command: {}", e)
                    })
                };
                
                Result::<_, Rejection>::Ok(warp::reply::json(&reply))
            }
        });
    println!("Server do be vibing on port 5000 tho 🎧");
    warp::serve(routes).run(([0, 0, 0, 0], 5000)).await;
}

fn is_docker_installed() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn is_container_image_present() -> bool {
    let output = Command::new("docker")
        .arg("images")
        .arg("--format")
        .arg("{{.Repository}}:{{.Tag}}")
        .output();

    match output {
        Ok(output) => {
            let images = String::from_utf8_lossy(&output.stdout);
            images.lines().any(|line| line == "docker.all-hands.dev/all-hands-ai/runtime:0.16-nikolaik")
        }
        Err(_) => false,
    }
}

async fn run_container() -> Result<(), String> {
    println!("Running the container...");
    let run_output = Command::new("sh")
        .arg("-c")
        .arg(&FULL_COMMAND.replace("~", &std::env::var("HOME").unwrap_or_default()))
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(|e| format!("Failed to execute docker run: {}", e))?;

    if !run_output.success() {
        return Err("Docker run failed.".to_string());
    }

    println!("Container is running.");
    Ok(())
}

async fn execute_full_command() -> Result<(), String> {
    println!("Executing FULL_COMMAND...");
    let command = FULL_COMMAND.replace("~", &std::env::var("HOME").unwrap_or_default());
    let full_output = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(|e| format!("Failed to execute FULL_COMMAND: {}", e))?;

    if !full_output.success() {
        return Err("FULL_COMMAND execution failed.".to_string());
    }

    println!("FULL_COMMAND executed successfully. Container is running.");
    Ok(())
}

fn install_docker() -> Result<(), String> {
    let os = std::env::consts::OS;
    let commands = match os {
        "linux" => vec![
            "sudo apt-get update",
            "sudo apt-get install -y apt-transport-https ca-certificates curl software-properties-common",
            "curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -",
            "sudo add-apt-repository 'deb [arch=amd64] https://download.docker.com/linux/ubuntu focal stable'",
            "sudo apt-get update",
            "sudo apt-get install -y docker-ce docker-ce-cli containerd.io",
        ],
        "macos" => vec![
            "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"",
            "brew install --cask docker",
        ],
        "windows" => vec![
            "choco install docker-desktop",
        ],
        _ => return Err(format!("Unsupported OS: {}", os)),
    };

    for cmd in commands {
        let result = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;
        if !result.status.success() {
            return Err(format!(
                "Command failed: {}\nError: {:?}",
                cmd,
                result.stderr
            ));
        }
    }

    Ok(())
}

fn run_command(command: &str) -> serde_json::Value {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output();

    match output {
        Ok(output) => serde_json::json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "status": output.status.code(),
        }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}