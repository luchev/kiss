use crate::grpc::immudb_grpc::SqlValue;
use config::ConfigError;
use error_chain::{error_chain, ExitCode};
use libp2p_identity::DecodingError;
use libp2p_kad::{store, GetRecordError, PutRecordError, QueryId};
use log::error;
use std::path::PathBuf;
use std::result;
use std::string::FromUtf8Error;
use std::time::SystemTimeError;
use std::{error::Error as StdError, io, process::exit};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use tokio::task::JoinError;
use tonic::metadata::errors::InvalidMetadataValue;
use tonic::Status;

use crate::types::{Bytes, SwarmInstruction};

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
        IoError(e: io::Error) { display("io error: {}", e) }
        StdError(e: String) { display("io error: {}", e) }
        TonicTransportError(e: tonic::transport::Error) { display("tonic transport error: {}", e) }
        SwarmOperationFailed(e: SendError<SwarmInstruction>) { display("swarm operation failed: {}", e) }
        SendPutReceiverFailed { display("send put receiver failed") }
        SendGetReceiverFailed { display("send get receiver failed") }
        ChannelReceiveError(e: RecvError) { display("channel receive failed: {}", e) }
        KademliaStoreError(e: store::Error) { display("kademlia store error: {}", e) }
        InjectorError(e: String) { display("injector error: {}", e) }
        InvalidResponseChannel(e: QueryId) { display("invalid response channel for query id {:?}", e) }
        MissingInstruction { display("no instruction provided") }
        SendingVectorResultFailed { display("sending vector result failed") }
        SendingEmptyResultFailed { display("sending empty result failed") }
        MutexIsEmpty { display("mutex not initialized correctly and is empty when unwrapping") }
        SettingsAddressesAreEmpty { display("settings addresses should have at least 1 address") }
        KeeperClientConnectionError { display("keeper client failed to connect") }
        JoinError(e: JoinError) { display("join error: {}", e) }
        InvalidTonicMetadataValue(e: InvalidMetadataValue) { display("invalid metadata: {}", e) }
        FailedTonicRequest(e: tonic::Status) { display("failed tonic request: {}", e) }
        MutexIsNotMutable { display("mutex not initialized correctly and is not mutable") }
        FailedConvertingFromUtf8(e: FromUtf8Error) { display("failed converting from utf-8: {}", e) }
        SystemTimeError(e: SystemTimeError) { display("system time error: {}", e) }
        InvalidSqlRow(e: Vec<SqlValue>) { display("invalid sql row: {:?}", e) }
        InvalidSql { display("invalid sql") }
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

impl From<std::io::Error> for Error {
    fn from(e: io::Error) -> Self {
        ErrorKind::IoError(e).into()
    }
}

impl From<tonic::transport::Error> for Error {
    fn from(e: tonic::transport::Error) -> Self {
        ErrorKind::TonicTransportError(e).into()
    }
}

impl From<SendError<SwarmInstruction>> for Error {
    fn from(e: SendError<SwarmInstruction>) -> Self {
        ErrorKind::SwarmOperationFailed(e).into()
    }
}

impl From<result::Result<Vec<u8>, Error>> for Error {
    fn from(_: result::Result<Vec<u8>, Error>) -> Self {
        ErrorKind::SendingVectorResultFailed.into()
    }
}

impl From<result::Result<(), Error>> for Error {
    fn from(_: result::Result<(), Error>) -> Self {
        ErrorKind::SendingEmptyResultFailed.into()
    }
}

impl From<RecvError> for Error {
    fn from(e: RecvError) -> Self {
        ErrorKind::ChannelReceiveError(e).into()
    }
}

impl From<oneshot::Receiver<std::result::Result<(), Error>>> for Error {
    fn from(_: oneshot::Receiver<std::result::Result<(), Error>>) -> Self {
        ErrorKind::SendPutReceiverFailed.into()
    }
}

impl From<store::Error> for Error {
    fn from(e: store::Error) -> Self {
        ErrorKind::KademliaStoreError(e).into()
    }
}

impl From<oneshot::Receiver<result::Result<Bytes, Error>>> for Error {
    fn from(_: oneshot::Receiver<result::Result<Bytes, Error>>) -> Self {
        ErrorKind::SendGetReceiverFailed.into()
    }
}

impl From<runtime_injector::InjectError> for Error {
    fn from(e: runtime_injector::InjectError) -> Self {
        ErrorKind::InjectorError(e.to_string()).into()
    }
}

impl From<InvalidMetadataValue> for Error {
    fn from(e: InvalidMetadataValue) -> Self {
        ErrorKind::InvalidTonicMetadataValue(e).into()
    }
}

impl From<tonic::Status> for Error {
    fn from(e: tonic::Status) -> Self {
        ErrorKind::FailedTonicRequest(e).into()
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self {
        ErrorKind::FailedConvertingFromUtf8(e).into()
    }
}

impl From<SystemTimeError> for Error {
    fn from(e: SystemTimeError) -> Self {
        ErrorKind::SystemTimeError(e).into()
    }
}

// impl From<&Vec<SqlValue>> for Error {
//     fn from(e: &Vec<SqlValue>) -> Self {
//         ErrorKind::InvalidSqlRow(e.clone()).into()
//     }
// }
