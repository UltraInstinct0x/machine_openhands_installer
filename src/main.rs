use std::process::Command;
use warp::Filter;

const FULL_COMMAND: &str = "
docker pull docker.all-hands.dev/all-hands-ai/runtime:0.16-nikolaik && \
docker run -it --rm --pull=always \
    -e SANDBOX_RUNTIME_CONTAINER_IMAGE=docker.all-hands.dev/all-hands-ai/runtime:0.16-nikolaik \
    -e LOG_ALL_EVENTS=true \
    -v /var/run/docker.sock:/var/run/docker.sock \
    -v ~/.openhands:/home/openhands/.openhands \
    -p 3000:3000 \
    --add-host host.docker.internal:host-gateway \
    --name openhands-app \
    docker.all-hands.dev/all-hands-ai/openhands:0.16
";

#[tokio::main]
async fn main() {
    // Install Docker if not installed
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

    // Check container image and decide to run directly or execute FULL_COMMAND
    println!("Checking if the container image exists...");
    if is_container_image_present() {
        println!("Container image found. Running container...");
        if let Err(e) = run_container() {
            eprintln!("Failed to run container: {}", e);
        }
    } else {
        println!("Container image not found. Executing FULL_COMMAND...");
        if let Err(e) = execute_full_command() {
            eprintln!("Failed to execute FULL_COMMAND: {}", e);
        }
    }

    // Start the server
    println!("Starting local command server...");
    let routes = warp::post()
        .and(warp::path("run"))
        .and(warp::body::json())
        .map(|cmd: serde_json::Value| {
            let output = run_command(cmd["command"].as_str().unwrap_or(""));
            warp::reply::json(&output)
        });

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

fn run_container() -> Result<(), String> {
    println!("Running the container...");
    let run_output = Command::new("sh")
        .arg("-c")
        .arg(FULL_COMMAND.split("&&").nth(1).unwrap_or("")) // Extract the docker run part
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(|e| format!("Failed to execute docker run: {}", e))?;

    if !run_output.success() {
        return Err("Docker run failed.".to_string());
    }

    println!("Container is running.");
    Ok(())
}

fn execute_full_command() -> Result<(), String> {
    println!("Executing FULL_COMMAND...");
    let full_output = Command::new("sh")
        .arg("-c")
        .arg(FULL_COMMAND)
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