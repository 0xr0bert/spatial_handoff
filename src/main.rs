use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::interval;

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

#[tokio::main]
async fn main() {
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

    // Bind UDP socket and wrap it in an Arc so it can be shared safely
    let socket = Arc::new(
        UdpSocket::bind(format!("127.0.0.1:{}", config.port))
            .await
            .expect("Failed to bind socket"),
    );

    // Create a channel to send handoff packets
    let (handoff_tx, mut handoff_rx) = mpsc::channel::<Agent>(100);

    // Network listener
    let listener_socket = Arc::clone(&socket);
    tokio::spawn(async move {
        let mut buffer = [0u8; 512];
        loop {
            if let Ok((size, _)) = listener_socket.recv_from(&mut buffer).await {
                if let Ok(incoming_agent) = serde_json::from_slice::<Agent>(&buffer[..size]) {
                    let _ = handoff_tx.send(incoming_agent).await;
                }
            }
        }
    });

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

    let mut tick_int = interval(Duration::from_secs(1));

    // core loop
    loop {
        tick_int.tick().await;

        // Process all incoming handoff packets
        while let Ok(incoming_agent) = handoff_rx.try_recv() {
            println!("Received handoff packet for agent {}", incoming_agent.id);
            local_agents.push(incoming_agent);
        }

        // simulate local agents and check boundaries
        let mut out_of_bounds_agents: Vec<Agent> = Vec::new();
        local_agents.retain_mut(|agent| {
            // apply velocity
            agent.x += agent.velocity_x;
            println!(
                "[{}] Agent {} moved to x: {}",
                config.name, agent.id, agent.x
            );

            if agent.x > config.boundary_max && config.name == "Node A" {
                out_of_bounds_agents.push(agent.clone());
                return false;
            } else if agent.x > config.boundary_max {
                println!("Agent {} exceeded world, back to beginning", agent.id);
                agent.x = 0.0f64;
                out_of_bounds_agents.push(agent.clone());
                return false;
            } else if agent.x < config.boundary_min && config.name == "Node B" {
                out_of_bounds_agents.push(agent.clone());
                return false;
            } else if agent.x < config.boundary_min {
                println!("Agent {} exceeded world, back to end", agent.id);
                agent.x = 100.0f64;
                out_of_bounds_agents.push(agent.clone());
                return false;
            }
            true
        });

        for agent in out_of_bounds_agents {
            println!("Initiated handoff for agent {}", agent.id);
            socket
                .send_to(
                    serde_json::to_vec(&agent).unwrap().as_slice(),
                    &config.neighbour_address,
                )
                .await
                .expect("Failed to send handoff packet");
        }
    }
}
