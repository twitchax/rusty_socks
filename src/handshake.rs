
pub struct Handshake {
    pub version: u8,
    pub num_methods: u8,
    pub methods: Vec<u8>
}

impl Handshake {
    pub fn from_data(data: &[u8]) -> Handshake {
        let version = data[0];
        let num_methods = data[1];
        let methods = data[2..(2 + usize::from(num_methods))].to_vec();

        Handshake { version, num_methods, methods }
    }
}