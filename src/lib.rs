extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
//#[macro_use]
//extern crate log;

#[derive(Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
    count: usize,
}


// Simulate some cpu-bound work, that does not involve shared
// state or blocking calls.
pub fn cpu_intensive_work() -> String {
    //info!("CPU_IN");
    let mut y = "X".to_string();
    let mut e = 10;
    for x in 0..1000 {
        y = format!("Value: {}", x);
        e = e + y.len();
    }
    let address = Address {
        street: "10 Downing Street".to_owned(),
        city: y.to_owned(),
        count: e,
    };

    let j = serde_json::to_string(&address).unwrap();
    //info!("CPU_OUT");
    return j;
}