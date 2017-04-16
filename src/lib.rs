extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

#[derive(Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
}


// Simulate some cpu-bound work, that does not involve shared
// state or blocking calls.
pub fn cpu_intensive_work() -> String {
    let mut y = "X".to_string();
    for x in 0..100 {
        y = format!("Value: {}", x);
    }
    let address = Address {
        street: "10 Downing Street".to_owned(),
        city: y.to_owned(),
    };

    let j = serde_json::to_string(&address).unwrap();
    return j;
}