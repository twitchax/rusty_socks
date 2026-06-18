use crate::helpers::{IntoError, Res};

pub struct Handshake {
    pub version: u8,
    pub num_methods: u8,
    pub methods: Vec<u8>,
}

impl Handshake {
    pub fn from_data(data: &[u8]) -> Res<Handshake> {
        // VER, NMETHODS.
        if data.len() < 2 {
            return "Handshake too short: need at least a version and a method count.".into_error();
        }

        let version = data[0];
        let num_methods = data[1];
        let methods_end = 2 + usize::from(num_methods);

        if data.len() < methods_end {
            return "Handshake too short for the stated number of methods.".into_error();
        }

        let methods = data[2..methods_end].to_vec();

        Ok(Handshake { version, num_methods, methods })
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
        let handshake = Handshake::from_data(&data).unwrap();

        assert_eq!(handshake.version, 5);
        assert_eq!(handshake.num_methods, 2);
        assert_eq!(handshake.methods, vec![0x00, 0x01]);
    }

    #[test]
    fn rejects_truncated_greeting() {
        // Fewer than two bytes: not even a version and method count.
        assert!(Handshake::from_data(&[0x05]).is_err());
    }

    #[test]
    fn rejects_greeting_missing_methods() {
        // Claims five methods but provides none.
        assert!(Handshake::from_data(&[0x05, 0x05]).is_err());
    }
}
