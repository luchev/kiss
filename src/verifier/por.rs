#![allow(clippy::all)]

use crate::util::types::{Bytes, Contract};

const P_BITS: u64 = 57;
const MIN_LOOP: usize = 8;
const P57: u64 = 144115188075855859;
const TINYMT_MASK: u64 = 0x7fffffffffffffff;
const BYTES_UNDER_P: usize = 7;
const CHUNK_ALIGN: usize = 56;

#[derive(Debug)]
pub struct VerificationClientConfig {
    rows: usize,
    cols: usize,
    secret_m_vector: Vec<u64>,
    secret_n_vector: Vec<u64>,
}

impl VerificationClientConfig {
    pub fn from_contract(contract: &Contract) -> Self {
        Self {
            rows: contract.rows as usize,
            cols: contract.cols as usize,
            secret_m_vector: bytes_to_u64(&contract.secret_m),
            secret_n_vector: bytes_to_u64(&contract.secret_n),
        }
    }

    pub fn from_contract_fields(secret_n: Bytes, secret_m: Bytes, rows: i64, cols: i64) -> Self {
        Self {
            rows: rows as usize,
            cols: cols as usize,
            secret_m_vector: bytes_to_u64(&secret_m),
            secret_n_vector: bytes_to_u64(&secret_n),
        }
    }

    pub fn to_contract(&self) -> (Bytes, Bytes, i64, i64) {
        (
            u64_to_bytes(self.secret_n_vector.clone()),
            u64_to_bytes(self.secret_m_vector.clone()),
            self.rows as i64,
            self.cols as i64,
        )
    }

    pub fn from_file(file: &Vec<u8>) -> Self {
        let num_chunks = 1 + (file.len() - 1) / BYTES_UNDER_P;
        let rows = (((num_chunks as f64).sqrt() / CHUNK_ALIGN as f64).ceil() * CHUNK_ALIGN as f64)
            as usize;
        let cols = 1 + (num_chunks - 1) / rows;

        let vector_u = Random::rand_vector(cols, 2020);

        let mut partials1 = vec![0_u128; rows];
        let bytes_per_row = BYTES_UNDER_P * rows;
        let chunk_mask = (1_u64 << (8 * BYTES_UNDER_P)) - 1;
        let file_extended: Vec<u8> = file
            .clone()
            .into_iter()
            .chain(vec![0; bytes_per_row])
            .collect();
        // file_extended.append(vec![0; bytes_per_row].as_mut());

        for i in 0..cols {
            let mut raw_ind = 0;
            let raw_row = file_extended[(bytes_per_row * i)..bytes_per_row * (i + 1)].to_vec();
            let raw_row = raw_row
                .chunks_exact(8)
                .map(|x| u64::from_le_bytes(x.try_into().unwrap_or_default()))
                .collect::<Vec<u64>>();
            for full_ind in (0..rows).step_by(8) {
                let mut data_val = (raw_row[raw_ind] & chunk_mask) as u128;
                partials1[full_ind] += data_val * vector_u[i] as u128;

                for k in 1..7 {
                    let data_val = ((raw_row[raw_ind + k - 1] >> (64 - k * 8))
                        | ((raw_row[raw_ind + k] << (k * 8)) & chunk_mask))
                        as u128;
                    partials1[full_ind + k] += data_val * vector_u[i] as u128;
                }

                data_val = (raw_row[raw_ind + 6] >> 8) as u128;
                partials1[full_ind + 7] += data_val * vector_u[i] as u128;

                raw_ind += 7;
            }
        }
        for k in 0..rows {
            partials1[k] %= P57 as u128;
        }

        Self {
            rows,
            cols,
            secret_m_vector: vector_u,
            secret_n_vector: partials1.iter().map(|x| *x as u64).collect::<Vec<u64>>(),
        }
    }
}

fn bytes_to_u64(bytes: &Bytes) -> Vec<u64> {
    bytes
        .chunks_exact(8)
        .map(|x| u64::from_le_bytes(x.try_into().unwrap()))
        .collect::<Vec<u64>>()
}

fn u64_to_bytes(u64s: Vec<u64>) -> Bytes {
    u64s.iter()
        .flat_map(|x| x.to_le_bytes().to_vec())
        .collect::<Bytes>()
}

#[derive(Debug)]
pub struct VerificationServerConfig {
    rows: usize,
    cols: usize,
    file: Vec<u8>,
}

impl VerificationServerConfig {
    pub fn from_file(file: Vec<u8>) -> Self {
        let num_chunks = 1 + (file.len() - 1) / BYTES_UNDER_P;
        let rows = (((num_chunks as f64).sqrt() / CHUNK_ALIGN as f64).ceil() * CHUNK_ALIGN as f64)
            as usize;
        let cols = 1 + (num_chunks - 1) / rows;
        Self { rows, cols, file }
    }
}

#[derive(Debug)]
pub struct VerificationClient {
    config: VerificationClientConfig,
}

impl VerificationClient {
    pub fn new(config: VerificationClientConfig) -> Self {
        Self { config }
    }

    pub fn make_challenge_vector(&self) -> Vec<u64> {
        Random::rand_vector(self.config.rows, 20)
    }

    pub fn audit(&self, challenge: Vec<u64>, response: Vec<u64>) -> bool {
        let mut rxr1: u128 = 0;
        let mut sxc1: u128 = 0;
        if response.len() != self.config.cols {
            return false;
        }
        if challenge.len() != self.config.rows {
            return false;
        }

        for i in 0..self.config.cols {
            rxr1 += response[i] as u128 * self.config.secret_m_vector[i] as u128;
            if rxr1 > P57 as u128 {
                rxr1 %= P57 as u128;
            }
        }

        for i in 0..self.config.rows {
            sxc1 += challenge[i] as u128 * self.config.secret_n_vector[i] as u128;
            if sxc1 > P57 as u128 {
                sxc1 %= P57 as u128;
            }
        }

        return rxr1 == sxc1;
    }
}

#[derive(Debug)]
pub struct VerificationServer {
    config: VerificationServerConfig,
}

impl VerificationServer {
    pub fn new(config: VerificationServerConfig) -> Self {
        Self { config }
    }

    pub fn fulfill_challenge(&self, challenge: Vec<u64>) -> Vec<u64> {
        let mut dot_prods1 = vec![0_u128; self.config.cols];
        let bytes_per_row = BYTES_UNDER_P * self.config.rows;
        let chunk_mask = (1_u64 << (8 * BYTES_UNDER_P)) - 1;
        let file_extended: Vec<u8> = self
            .config
            .file
            .clone()
            .into_iter()
            .chain(vec![0; bytes_per_row])
            .collect();

        for i in 0..self.config.cols {
            let mut raw_ind = 0;
            let raw_row = file_extended[(bytes_per_row * i)..bytes_per_row * (i + 1)].to_vec();
            let raw_row = raw_row
                .chunks_exact(8)
                .map(|x| u64::from_le_bytes(x.try_into().unwrap()))
                .collect::<Vec<u64>>();
            for full_ind in (0..self.config.rows).step_by(8) {
                let mut data_val = (raw_row[raw_ind] & chunk_mask) as u128;
                dot_prods1[i] += data_val * challenge[full_ind] as u128;

                for k in 1..7 {
                    let data_val = ((raw_row[raw_ind + k - 1] >> (64 - k * 8))
                        | ((raw_row[raw_ind + k] << (k * 8)) & chunk_mask))
                        as u128;
                    dot_prods1[i] += data_val * challenge[full_ind + k] as u128;
                }

                data_val = (raw_row[raw_ind + 6] >> 8) as u128;
                dot_prods1[i] += data_val * challenge[full_ind + 7] as u128;

                raw_ind += 7;
            }
        }
        for k in 0..self.config.cols {
            dot_prods1[k] %= P57 as u128;
        }

        return dot_prods1.iter().map(|x| *x as u64).collect::<Vec<u64>>();
    }
}

struct Random {
    status: [u64; 2],
    mat1: u32,
    mat2: u32,
    tmat: u64,
}

impl Random {
    fn init(&mut self, seed: u64) {
        self.status[0] = seed ^ ((self.mat1 as u64) << 32);
        self.status[1] = self.mat2 as u64 ^ self.tmat;
        for i in 1..MIN_LOOP {
            self.status[i & 1] ^= (i as u128
                + 6364136223846793005_u128
                    * ((self.status[(i - 1) & 1] ^ (self.status[(i - 1) & 1] >> 62)) as u128))
                as u64;
        }
        self.period_certification();
    }

    fn period_certification(&mut self) {
        if (self.status[0] & TINYMT_MASK) == 0 && self.status[1] == 0 {
            self.status[0] = 'T' as u64;
            self.status[1] = 'M' as u64;
        }
    }

    fn rand_mod_p(&mut self) -> u64 {
        let mask = (1_u64 << P_BITS) - 1;
        let mut val: u64;
        loop {
            val = self.generate_uint64() & mask;
            if val < P57 {
                break;
            }
        }
        val
    }

    fn generate_uint64(&mut self) -> u64 {
        self.next_state();
        self.temper()
    }

    fn temper(&mut self) -> u64 {
        let mut x: u64;
        x = ((self.status[0] as u128 + self.status[1] as u128) % u64::MAX as u128) as u64;
        x ^= self.status[0] >> 8;
        x ^= (-((x & 1) as i64) & self.tmat as i64) as u64;
        x
    }

    fn next_state(&mut self) {
        let mut x: u64;
        self.status[0] &= TINYMT_MASK;
        x = self.status[0] ^ self.status[1];
        x ^= x << 12;
        x ^= x >> 32;
        x ^= x << 32;
        x ^= x << 11;
        self.status[0] = self.status[1];
        self.status[1] = x;
        self.status[0] ^= (-((x & 1) as i64) & self.mat1 as i64) as u64;
        self.status[1] ^= (-((x & 1) as i64) & ((self.mat2 as u64) << 32) as i64) as u64;
    }

    fn rand_vector(size: usize, seed: u64) -> Vec<u64> {
        let mut state = Random {
            status: [0; 2],
            mat1: 0,
            mat2: 0,
            tmat: 0,
        };
        state.init(seed);

        let mut vector = vec![0; size];
        for i in 0..size {
            vector[i] = state.rand_mod_p();
        }
        vector
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    extern crate test;
    use test::{black_box, Bencher};

    fn audit(client: &VerificationClient, server: &VerificationServer) -> bool {
        let challenge = client.make_challenge_vector();
        let response = server.fulfill_challenge(challenge.clone());
        client.audit(challenge, response)
    }

    fn init(file: Vec<u8>) -> (VerificationClientConfig, VerificationServerConfig) {
        let num_chunks = 1 + (file.len() - 1) / BYTES_UNDER_P;
        let rows = (((num_chunks as f64).sqrt() / CHUNK_ALIGN as f64).ceil() * CHUNK_ALIGN as f64)
            as usize;
        let cols = 1 + (num_chunks - 1) / rows;

        let vector_u = Random::rand_vector(cols, 2020);

        let mut partials1 = vec![0_u128; rows];
        let bytes_per_row = BYTES_UNDER_P * rows;
        let chunk_mask = (1_u64 << (8 * BYTES_UNDER_P)) - 1;
        let file_extended: Vec<u8> = file
            .clone()
            .into_iter()
            .chain(vec![0; bytes_per_row])
            .collect();
        // file_extended.append(vec![0; bytes_per_row].as_mut());

        for i in 0..cols {
            let mut raw_ind = 0;
            let raw_row = file_extended[(bytes_per_row * i)..bytes_per_row * (i + 1)].to_vec();
            let raw_row = raw_row
                .chunks_exact(8)
                .map(|x| u64::from_le_bytes(x.try_into().unwrap_or_default()))
                .collect::<Vec<u64>>();
            for full_ind in (0..rows).step_by(8) {
                let mut data_val = (raw_row[raw_ind] & chunk_mask) as u128;
                partials1[full_ind] += data_val * vector_u[i] as u128;

                for k in 1..7 {
                    let data_val = ((raw_row[raw_ind + k - 1] >> (64 - k * 8))
                        | ((raw_row[raw_ind + k] << (k * 8)) & chunk_mask))
                        as u128;
                    partials1[full_ind + k] += data_val * vector_u[i] as u128;
                }

                data_val = (raw_row[raw_ind + 6] >> 8) as u128;
                partials1[full_ind + 7] += data_val * vector_u[i] as u128;

                raw_ind += 7;
            }
        }
        for k in 0..rows {
            partials1[k] %= P57 as u128;
        }

        let client_config = VerificationClientConfig {
            rows,
            cols,
            secret_m_vector: vector_u,
            secret_n_vector: partials1.iter().map(|x| *x as u64).collect::<Vec<u64>>(),
        };

        let server_config = VerificationServerConfig { rows, cols, file };

        (client_config, server_config)
    }

    #[test]
    fn test_init() {
        let (client_config, server_config) = init("abcdefghijklmnopqrstuvwxyz".as_bytes().to_vec());
        assert_eq!(client_config.rows, 56);
        assert_eq!(client_config.cols, 1);
        assert_eq!(client_config.secret_m_vector, vec![57829946736570845]);
        assert_eq!(
            client_config.secret_n_vector,
            vec![
                120891374367124132,
                131035456404565768,
                141179538442007404,
                22254200296736808,
            ]
            .into_iter()
            .chain(vec![0; 52].into_iter())
            .collect::<Vec<u64>>()
        );
        assert_eq!(server_config.rows, 56);
        assert_eq!(server_config.cols, 1);
        assert_eq!(
            server_config.file,
            "abcdefghijklmnopqrstuvwxyz".as_bytes().to_vec()
        );
    }

    #[test]
    fn test_audit() {
        let (client_config, server_config) = init("abcdefghijklmnopqrstuvwxyz".as_bytes().to_vec());
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        assert!(audit(&client, &server));
    }

    #[test]
    fn test_audit_separate() {
        let file = "abcdefghijklmnopqrstuvwxyz".as_bytes().to_vec();
        let client_config = VerificationClientConfig::from_file(&file);
        let server_config = VerificationServerConfig::from_file(file);
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        assert!(audit(&client, &server));
    }

    #[test]
    fn test_audit_abc_for_debugging() {
        let file = "abc".as_bytes().to_vec();
        let client_config = VerificationClientConfig::from_file(&file);
        let server_config = VerificationServerConfig::from_file(file);
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        let challenge = client.make_challenge_vector();
        let response = server.fulfill_challenge(challenge.clone());
        client.audit(challenge, response);
    }

    #[test]
    fn test_audit_10mb() {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(400000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        assert!(audit(&client, &server));
    }

    #[test]
    fn test_audit_100mb() {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(4000000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        assert!(audit(&client, &server));
    }

    #[bench]
    fn bench_init_1mb(b: &mut Bencher) {
        b.iter(|| {
            black_box(init(
                "abcdefghijklmnopqrstuvwxyz"
                    .as_bytes()
                    .to_vec()
                    .repeat(40000 * 1),
            ));
        });
    }

    #[bench]
    fn bench_init_10mb(b: &mut Bencher) {
        b.iter(|| {
            black_box(init(
                "abcdefghijklmnopqrstuvwxyz"
                    .as_bytes()
                    .to_vec()
                    .repeat(400000),
            ));
        });
    }

    #[bench]
    fn bench_init_100mb(b: &mut Bencher) {
        b.iter(|| {
            black_box(init(
                "abcdefghijklmnopqrstuvwxyz"
                    .as_bytes()
                    .to_vec()
                    .repeat(4000000),
            ));
        });
    }

    #[bench]
    fn bench_100mb_init_random(b: &mut Bencher) {
        let size_in_bytes = 1024 * 1024 * 100;
        let mut rng = rand::thread_rng();
        let random_bytes: Vec<u8> = (0..size_in_bytes).map(|_| rng.gen()).collect();

        b.iter(|| {
            black_box(init(random_bytes.clone()));
        });
    }

    // #[bench]
    // fn bench_init_1gb(b: &mut Bencher) {
    //     b.iter(|| {
    //         black_box(init(
    //             "abcdefghijklmnopqrstuvwxyz"
    //                 .as_bytes()
    //                 .to_vec()
    //                 .repeat(40000000),
    //         ));
    //     });
    // }

    #[bench]
    fn bench_audit_1mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(40000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        b.iter(|| {
            black_box(audit(&client, &server));
        });
    }

    #[bench]
    fn bench_audit_10mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(400000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        b.iter(|| {
            black_box(audit(&client, &server));
        });
    }

    #[bench]
    fn bench_audit_100mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(4000000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        b.iter(|| {
            black_box(audit(&client, &server));
        });
    }

    #[bench]
    fn bench_client_make_challenge_1mb(b: &mut Bencher) {
        let (client_config, _) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(40000 * 1),
        );
        let client = VerificationClient::new(client_config);
        b.iter(|| black_box(client.make_challenge_vector()));
    }

    #[bench]
    fn bench_client_make_challenge_10mb(b: &mut Bencher) {
        let (client_config, _) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(400000),
        );
        let client = VerificationClient::new(client_config);
        b.iter(|| black_box(client.make_challenge_vector()));
    }

    #[bench]
    fn bench_client_make_challenge_100mb(b: &mut Bencher) {
        let (client_config, _) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(4000000),
        );
        let client = VerificationClient::new(client_config);
        b.iter(|| black_box(client.make_challenge_vector()));
    }

    #[bench]
    fn bench_audit_client_1mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(40000 * 1),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        let challenge = client.make_challenge_vector();
        let response = server.fulfill_challenge(challenge.clone());
        b.iter(|| black_box(client.audit(challenge.clone(), response.clone())));
    }

    #[bench]
    fn bench_audit_client_10mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(400000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        let challenge = client.make_challenge_vector();
        let response = server.fulfill_challenge(challenge.clone());
        b.iter(|| black_box(client.audit(challenge.clone(), response.clone())));
    }

    #[bench]
    fn bench_audit_client_100mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(4000000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        let challenge = client.make_challenge_vector();
        let response = server.fulfill_challenge(challenge.clone());
        b.iter(|| black_box(client.audit(challenge.clone(), response.clone())));
    }

    #[bench]
    fn bench_server_response_1mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(40000 * 100),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        let challenge = client.make_challenge_vector();
        b.iter(|| black_box(server.fulfill_challenge(challenge.clone())));
    }

    #[bench]
    fn bench_server_response_10mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(400000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        let challenge = client.make_challenge_vector();
        b.iter(|| black_box(server.fulfill_challenge(challenge.clone())));
    }

    #[bench]
    fn bench_server_response_100mb(b: &mut Bencher) {
        let (client_config, server_config) = init(
            "abcdefghijklmnopqrstuvwxyz"
                .as_bytes()
                .to_vec()
                .repeat(40000000),
        );
        let client = VerificationClient::new(client_config);
        let server = VerificationServer::new(server_config);
        let challenge = client.make_challenge_vector();
        b.iter(|| black_box(server.fulfill_challenge(challenge.clone())));
    }
}
