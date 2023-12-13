use coap::{CoAPClient};
use serde_json::json;
use rand::Rng;

fn main() {
    let url_register = "coap://127.0.0.1:5683/sensor/register";
    let url_read = "coap://127.0.0.1:5683/sensor";

    let sensor_type = "current";

    let register_params = json! {
        {
            "sensor_type": sensor_type,
            "ip_address": "127.0.0.1"
        }
    }.to_string().as_bytes().to_vec();

    let response_register = CoAPClient::post(url_register, register_params).unwrap();
    let new_sensor = String::from_utf8(response_register.message.payload).unwrap();

    let sensor_id = new_sensor.parse::<i32>().unwrap();

    loop {
        println!("Reading sensor: {}", new_sensor);

        let sensor_value = rand::thread_rng().gen_range(0.0f32..20.0f32);

        let read_params = json! {
            {
                "sensor_id": sensor_id,
                "sensor_value": sensor_value
            }
        }.to_string().as_bytes().to_vec();

        let response_read = CoAPClient::post(url_read, read_params).unwrap();
        println!("Server reply: {}", String::from_utf8(response_read.message.payload).unwrap());

        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}
