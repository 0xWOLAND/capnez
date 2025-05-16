use capnp_derive::CapnpDerive;

#[derive(CapnpDerive)]
struct Home {
    owner: Person,
    address: String,
    rooms: Vec<Room>,
    security_system: SecuritySystem,
}

#[derive(CapnpDerive)]
struct Person {
    name: String,
    age: u32,
    contact_info: ContactInfo,
}

#[derive(CapnpDerive)]
struct ContactInfo {
    email: String,
    phone: String,
}

#[derive(CapnpDerive)]
struct Room {
    name: String,
    devices: Vec<Device>,
}

#[derive(CapnpDerive)]
struct Device {
    id: String,
    kind: DeviceKind,
    status: DeviceStatus,
}

#[derive(CapnpDerive)]
enum DeviceKind {
    Thermostat,
    Light,
    Camera,
    Lock,
}

#[derive(CapnpDerive)]
struct DeviceStatus {
    online: bool,
    battery_level: u8,
}

#[derive(CapnpDerive)]
struct SecuritySystem {
    armed: bool,
    cameras: Vec<Device>,
}

fn main() {
    println!("Capnp schema generated!");
}
