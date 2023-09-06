use config::ConfigError;
use error_chain::{error_chain, ExitCode};
use libp2p_identity::DecodingError;
use libp2p_kad::{GetRecordError, PutRecordError};
use log::error;
use std::path::PathBuf;
use std::{error::Error as StdError, process::exit};
use tonic::Status;

trait ErrorHelper {
    fn help(&self) -> String;
}

pub trait Die {
    fn die(self);
}

error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }
    errors {
        UnknownError { display("unknown error") }
        DockerConnectionFailed(e: String) { display("could not connect to docker unix socket: {}", e) }
        LocalStorageFail(e: object_store::Error) { display("local storage failure: {}", e) }
        ConfigErr(e: ConfigError) { display("loading config failed: {}", e) }
        SettingsDependencyFail { display("") }
        SettingsParseError(e: String) { display("") }
        StoragePutFailed(e: object_store::Error) { display("storing file failed: {}", e) }
        StorageGetFailed(e: object_store::Error) { display("retrieving file failed: {}", e) }
        StorageConvertToStreamFailed(e: object_store::Error) { display("converting file to stream failed: {}", e) }
        GrpcServerStartFailed(e: tonic::transport::Error) {
            display("grpc server failed to start: {}", e.source().map_or("unknown transport error".to_string(), |e| e.to_string())),
        }
        KeypairProtobufDecodeError(e: DecodingError) { display("decoding keypair error: {}", e) }
        KeypairBase64DecodeError(e: base64::DecodeError) { display("keypair decode error: {}", e) }
        KeypairBase64DecodingError(e: libp2p_identity::DecodingError) { display("keypair decoding error: {}", e) }
        SwarmPutRecordError(e: PutRecordError) { display("putting record to swarm failed: {}", e) }
        SwarmGetRecordError(e: GetRecordError) { display("getting record from swarm failed: {}", e) }
        SwarmGetRecordUnknownError(e: String) { display("getting record from swarm failed: {}", e) }
        GrpcError(e: Status) { display("grpc error: {}", e) }
        PathParsingError(e: PathBuf) { display("unable to parse path: {}", e.display()) }
        BehaviourInitFailed(e: std::io::Error) { display("p2p behaviour init failed: {}", e) }
        NoiseInitFailed(e: libp2p::noise::Error) { display("p2p noise init failed: {}", e) }
        IpParseFailed(e: libp2p::multiaddr::Error) { display("p2p ip address failed: {}", e) }
        SwarmListenFailed(e: libp2p::TransportError<std::io::Error>) { display("p2p listen failed: {}", e) }
    }
}

impl ExitCode for Error {
    fn code(self) -> i32 {
        match self.0 {
            ErrorKind::DockerConnectionFailed(_) => 1,
            _ => 1,
        }
    }
}

impl ErrorHelper for Error {
    fn help(&self) -> String {
        match self.0 {
            ErrorKind::DockerConnectionFailed(_) => "Is Docker running?",
            ErrorKind::LocalStorageFail(_) => "Does the directory exist?",
            _ => "No help available for this error",
        }
        .to_string()
    }
}

impl Die for Error {
    fn die(self) {
        die(self);
    }
}

pub fn die(err: Error) {
    error!("{}", err);
    error!("{}", err.help());
    exit(err.code());
}
