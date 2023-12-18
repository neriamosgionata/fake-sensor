use coap::{CoAPClient};
use serde_json::json;
use rand::Rng;
use local_ip_address::local_ip;
use fake_sensor::sensor_types::SENSOR_TYPE_CURRENT;

fn main() {
    let url_register = "coap://127.0.0.1:5683/sensor/register";
    let url_read = "coap://127.0.0.1:5683/sensor";

    let mut sensor_ip_address = String::new();
    let sensor_type = SENSOR_TYPE_CURRENT;

    match local_ip() {
        Ok(ip) => {
            sensor_ip_address.push_str(ip.to_string().as_str());
        }
        Err(_) => panic!("Unable to get local IP address")
    }

    println!("Local IP address: {}", sensor_ip_address);

    let register_params = json! {
        {
            "sensor_type": sensor_type,
            "ip_address": sensor_ip_address
        }
    }.to_string().as_bytes().to_vec();

    let response_register = CoAPClient::post(url_register, register_params).unwrap();
    let new_sensor = String::from_utf8(response_register.message.payload).unwrap();

    if new_sensor == "KO" {
        println!("Error registering sensor");
        return;
    }

    let sensor_id = new_sensor.parse::<i32>().unwrap();

    loop {
        read_sensor(&new_sensor, sensor_id, url_read);
    }
}

fn read_sensor(new_sensor: &String, sensor_id: i32, url_read: &str) {
    println!("Reading sensor: {}", new_sensor);

    let sensor_value = rand::thread_rng().gen_range(0.0f32..20.0f32).to_string();

    let read_params = json! {
            {
                "sensor_id": sensor_id,
                "sensor_value": sensor_value
            }
        }.to_string().as_bytes().to_vec();

    match CoAPClient::post(url_read, read_params) {
        Ok(response_read) => {
            println!("Server reply: {}", String::from_utf8(response_read.message.payload).unwrap());
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        Err(e) => {
            println!("Server error: {:?}", e);
            println!("Waiting for server to comeback");

            let mut i = 10;

            loop {
                if i == 0 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
                println!("{}...", i);
                i -= 1;
            }
        }
    }
}
