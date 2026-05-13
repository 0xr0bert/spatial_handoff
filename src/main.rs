use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Agent {
    id: u32,
    x: f64,
    velocity_x: f64,
}

struct NodeConfig {
    name: String,
    port: u16,
    neighbour_address: String,
    boundary_min: f64,
    boundary_max: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: cargo run -- [a|b]");
        return;
    }

    // Hardcoded config for silly PoC
    let config = match args[1].as_str() {
        "a" => NodeConfig {
            name: "Node A".to_string(),
            port: 8081,
            neighbour_address: "127.0.0.1:8082".to_string(),
            boundary_min: 0.0,
            boundary_max: 50.0,
        },
        "b" => NodeConfig {
            name: "Node B".to_string(),
            port: 8082,
            neighbour_address: "127.0.0.1:8081".to_string(),
            boundary_min: 50.0,
            boundary_max: 100.0,
        },
        _ => panic!("Invalid node name"),
    };

    // Bind UDP socket and set it to non-blocking so sim loop doesn't freeze
    let socket = std::net::UdpSocket::bind(format!("127.0.0.1:{}", config.port))
        .expect("Failed to bind socket");

    socket
        .set_nonblocking(true)
        .expect("Failed to set socket to non-blocking");

    let mut local_agents: Vec<Agent> = Vec::new();

    // Node A starts with 1 agent. Node B starts empty
    if config.name == "Node A" {
        local_agents.push(Agent {
            id: 1,
            x: 5.0,
            velocity_x: 10.0,
        });
        println!("Node A spawned agent 1");
    }

    println!(
        "Node {} started with {} agents",
        config.name,
        local_agents.len()
    );

    let mut buffer = [0u8; 512];

    // core loop
    loop {
        // process incoming packets
        while let Ok((size, _)) = socket.recv_from(&mut buffer) {
            if let Ok(incoming_agent) = serde_json::from_slice::<Agent>(&buffer[..size]) {
                println!(
                    "Received handoff: assumed authority of agent {}",
                    incoming_agent.id
                );
                local_agents.push(incoming_agent);
            }
        }

        // simulate local agents and check boundaries
        local_agents.retain_mut(|agent| {
            // apply velocity
            agent.x += agent.velocity_x;
            println!(
                "[{}] Agent {} moved to x: {}",
                config.name, agent.id, agent.x
            );

            if agent.x > config.boundary_max && config.name == "Node A" {
                println!(
                    "Initiated handoff: agent {} exceeded boundary; transferring to {}",
                    agent.id, config.neighbour_address
                );
                let _ = socket.send_to(
                    serde_json::to_vec(&agent).unwrap().as_slice(),
                    &config.neighbour_address,
                );
                return false;
            } else if agent.x > config.boundary_max {
                println!("Agent {} exceeded boundary", agent.id);
                return false;
            }
            true
        });

        thread::sleep(Duration::from_secs(1));
    }
}
