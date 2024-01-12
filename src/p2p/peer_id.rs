use base64::Engine;
use libp2p_identity::{ed25519, Keypair};
use log::debug;

use crate::util::Res;

fn generate_keypair() -> Keypair {
    Keypair::generate_ed25519()
}

fn keypair_to_base64_proto(keypair: Keypair) -> String {
    base64::engine::general_purpose::STANDARD_NO_PAD
        .encode(keypair.to_protobuf_encoding().unwrap_or_default())
}

fn keypair_from_base64_proto(encoded: String) -> Res<Keypair> {
    let decoded = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(encoded.as_bytes())
        .unwrap_or_default();
    Keypair::from_protobuf_encoding(&decoded).map_err(|err| err.into())
}

fn generate_with_leading_zeros(leading_zeros: usize) -> Keypair {
    let mut tries = 0;
    let keypair = loop {
        tries += 1;
        let inner = ed25519::Keypair::generate();
        let hashed = crate::util::hasher::hash(inner.public().to_bytes().as_slice());
        if hashed.chars().take(leading_zeros).all(|c| c == '0') {
            break Keypair::from(inner);
        }
    };
    debug!("generated keypair after {} tries", tries);
    keypair
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::hasher::hash;
    use rand::Rng;
    extern crate test;
    use test::{black_box, Bencher};

    #[test]
    fn test_generate_keypair() {}

    #[test]
    fn test_encode_decode() {
        let keypair = Keypair::generate_ed25519();
        let encoded = keypair_to_base64_proto(keypair.clone());
        let decoded = keypair_from_base64_proto(encoded);

        let original = keypair.try_into_ed25519().unwrap();
        let decoded = decoded.unwrap().try_into_ed25519().unwrap();

        assert_eq!(original.public(), decoded.public());
        assert_eq!(original.secret().as_ref(), decoded.secret().as_ref());
    }

    #[test]
    fn with_leading_zeros() {
        env_logger::init();
        let keypair = generate_with_leading_zeros(2);
        let hashed = hash(
            keypair
                .public()
                .try_into_ed25519()
                .unwrap()
                .to_bytes()
                .as_slice(),
        );
        assert!(hashed.chars().take(2).all(|c| c == '0'));
    }

    #[bench]
    fn bench_peer_id_1_leading_zero(b: &mut Bencher) {
        b.iter(|| {
            black_box(generate_with_leading_zeros(1));
        })
    }

    #[bench]
    fn bench_peer_id_2_leading_zero(b: &mut Bencher) {
        b.iter(|| {
            black_box(generate_with_leading_zeros(2));
        })
    }

    #[bench]
    fn bench_peer_id_3_leading_zero(b: &mut Bencher) {
        b.iter(|| {
            black_box(generate_with_leading_zeros(3));
        })
    }

    // #[bench]
    // fn bench_peer_id_4_leading_zero(b: &mut Bencher) {
    //     b.iter(|| {
    //         black_box(generate_with_leading_zeros(4));
    //     })
    // }
}
