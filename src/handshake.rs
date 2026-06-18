pub struct Handshake {
    pub version: u8,
    pub num_methods: u8,
    pub methods: Vec<u8>,
}

impl Handshake {
    pub fn from_data(data: &[u8]) -> Handshake {
        let version = data[0];
        let num_methods = data[1];
        let methods = data[2..(2 + usize::from(num_methods))].to_vec();

        Handshake { version, num_methods, methods }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_greeting() {
        // VER=5, NMETHODS=2, METHODS=[NO_AUTH, GSSAPI].
        let data = [0x05, 0x02, 0x00, 0x01];
        let handshake = Handshake::from_data(&data);

        assert_eq!(handshake.version, 5);
        assert_eq!(handshake.num_methods, 2);
        assert_eq!(handshake.methods, vec![0x00, 0x01]);
    }
}
