use std::thread::spawn;
use std::time::Duration;
use serde_json::json;
use rand::Rng;
use local_ip_address::local_ip;
use fake_sensor::{CoAPClient, Server};
use tokio::runtime::Runtime;
use coap_lite::{CoapRequest, MessageType, RequestType};
use std::net::SocketAddr;

fn discover_backend(default_port: u16) -> (String, u16) {
    println!("Discovering backend via CoAP multicast...");

    let client = match CoAPClient::new(("224.0.1.187", default_port)) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to create discovery client: {:?}, using fallback", e);
            return ("127.0.0.1".to_string(), default_port);
        }
    };

    client.set_broadcast(true).ok();
    let _ = client.set_receive_timeout(Some(Duration::from_secs(3)));

    let mut request = CoapRequest::<SocketAddr>::new();
    request.set_method(RequestType::Get);
    request.set_path("/.well-known/core");
    request
        .message
        .header
        .set_type(MessageType::NonConfirmable);

    if let Err(e) = client.send_all_coap(&request, 0) {
        println!("Failed to send multicast: {:?}, using fallback", e);
        return ("127.0.0.1".to_string(), default_port);
    }

    match client.receive() {
        Ok(response) => {
            let payload = String::from_utf8_lossy(&response.message.payload);
            println!("Discovery response: {}", payload);
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&payload) {
                let ip = parsed["ip"].as_str().unwrap_or("127.0.0.1").to_string();
                let port = parsed["coap_port"].as_u64().unwrap_or(default_port as u64) as u16;
                println!("Discovered backend at {}:{}", ip, port);
                return (ip, port);
            }
            println!("Failed to parse discovery response, using fallback");
            ("127.0.0.1".to_string(), default_port)
        }
        Err(e) => {
            println!("No discovery response: {:?}, using fallback", e);
            ("127.0.0.1".to_string(), default_port)
        }
    }
}

fn main() {
    let (backend_ip, backend_port) = discover_backend(5683);
    let url_register = format!("coap://{}:{}/sensor/register", backend_ip, backend_port);
    let url_read = format!("coap://{}:{}/sensor", backend_ip, backend_port);

    let mut sensor_ip_address = String::new();
    let sensor_port: i16 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "8685".to_string())
        .parse()
        .expect("Invalid port number");
    let sensor_type_arg = std::env::args().nth(2).unwrap_or_else(|| "current".to_string());
    let sensor_type: &str = Box::leak(sensor_type_arg.into_boxed_str());
    let device_secret = std::env::args().nth(3).unwrap_or_else(|| {
        eprintln!("Warning: no device_secret provided (arg 3)");
        String::new()
    });

    match local_ip() {
        Ok(ip) => {
            sensor_ip_address.push_str(ip.to_string().as_str());
        }
        Err(_) => panic!("Unable to get local IP address")
    }

    println!("Local IP address: {}", sensor_ip_address);

    let register_params = json! {
        {
            "device_secret": device_secret,
            "sensor_type": sensor_type,
            "ip_address": sensor_ip_address,
            "port": sensor_port,
            "online": true,
        }
    }.to_string().as_bytes().to_vec();

    let response_register = CoAPClient::post(&url_register, register_params.clone()).unwrap();
    let new_sensor = String::from_utf8(response_register.message.payload).unwrap();

    if new_sensor == "KO" || new_sensor == "Unauthorized" {
        println!("Error registering sensor: {}", new_sensor);
        return;
    }

    let sensor_id = new_sensor.parse::<i32>().unwrap();

    spawn(move || {
        Runtime::new()
            .unwrap()
            .block_on(run_server(sensor_ip_address, sensor_port));
    });

    loop {
        read_sensor(&new_sensor, sensor_id, &device_secret, &url_read, &url_register, register_params.clone());
    }
}

fn retry(url_register: &str, register_params: Vec<u8>) {
    let mut i = 10;

    loop {
        if i == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!("{}...", i);
        i -= 1;
    }

    let response_register = CoAPClient::post(url_register, register_params.clone());

    match response_register {
        Ok(_) => {
            println!("Sensor re-registered");
        }
        Err(_) => {
            std::thread::sleep(std::time::Duration::from_secs(59));
        }
    }
}

fn read_sensor(new_sensor: &String, sensor_id: i32, device_secret: &str, url_read: &str, url_register: &str, register_params: Vec<u8>) {
    println!("Reading sensor: {}", new_sensor);

    let sensor_value = rand::thread_rng().gen_range(0.0f32..20.0f32).to_string();

    let read_params = json! {
            {
                "device_secret": device_secret,
                "sensor_id": sensor_id,
                "sensor_value": sensor_value
            }
        }.to_string().as_bytes().to_vec();

    match CoAPClient::post(url_read, read_params) {
        Ok(response_read) => {
            let reply = String::from_utf8(response_read.message.payload).unwrap();
            println!("Server reply: {}", reply);

            if reply.contains("KO") {
                retry(url_register, register_params.clone());
            }

            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        Err(e) => {
            println!("Server error: {:?}", e);
            println!("Waiting for server to comeback");

            retry(url_register, register_params.clone());
        }
    }
}

async fn run_server(actuator_ip_address: String, actuator_port: i16) {
    let address = actuator_ip_address + ":" + actuator_port.to_string().as_str();

    println!("Running server on {}", address);

    let mut server = Server::new(address).unwrap();

    server.run(
        |request| async move {
            let payload = String::from_utf8(request.message.payload.clone()).unwrap();

            let mut return_payload = String::from("ON");

            if payload == "READ" {
                return_payload = rand::thread_rng().gen_range(0.0f32..20.0f32).to_string();
            }

            match request.response {
                Some(mut message) => {
                    message.message.payload = return_payload.as_bytes().to_vec();
                    Some(message)
                }
                _ => None,
            }
        },
    )
        .await
        .expect("Failed to create server");
}
