use crate::p2p::swarm::{QueryGetResponse, VerificationResponse};
use crate::util::grpc::immudb_grpc::SqlValue;
use config::ConfigError;
use error_chain::{error_chain, ExitCode};
use libp2p::request_response::{InboundFailure, OutboundFailure, RequestId};
use libp2p_identity::{DecodingError, ParseError, PeerId};
use libp2p_kad::{
    store, AddProviderError, GetClosestPeersError, GetProvidersError, GetRecordError,
    PutRecordError, QueryId,
};
use log::error;
use std::collections::HashSet;
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

use crate::util::types::{Bytes, CommandToSwarm};

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
        IoDetailed(e: io::Error, port: u16) { display("io error on port {}: {}", port, e) }
        DockerConnectionFailed(e: String) { display("could not connect to docker unix socket: {}", e) }
        LocalStorageFail(e: object_store::Error) { display("local storage failure: {}", e) }
        FilesystemErr(e: io::Error) { display("directory creation failed: {}", e) }
        ConfigErr(e: ConfigError) { display("loading config failed: {}", e) }
        Generic(e: String) { display("{}", e) }
        SettingsDependencyFail { display("") }
        SettingsParseError(e: String) { display("") }
        ObjectStoreError(e: object_store::Error) { display("object store error: {}", e) }
        StoragePutSerdeError(e: serde_yaml::Error) { display("storing file failed due to serde: {}", e) }
        StorageGetSerdeError(e: serde_yaml::Error) { display("getting file failed due to serde: {}", e) }
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
        SwarmGetProvidersError(e: GetProvidersError) { display("getting providers failed: {}", e) }
        SwarmStartProvidingError(e: AddProviderError) { display("add provider failed: {}", e) }
        SwarmGetClosestPeersError(e: GetClosestPeersError) { display("get closest peers failed: {}", e) }
        SwarmGetRecordUnknownError(e: String) { display("getting record from swarm failed: {}", e) }
        GrpcError(e: Status) { display("grpc error: {}", e) }
        PathParsingError(e: PathBuf) { display("unable to parse path: {}", e.display()) }
        RequestOutboundFailure(e: OutboundFailure) { display("outbound failure: {}", e) }
        RequestInboundFailure(e: InboundFailure) { display("inbound failure: {}", e) }
        BehaviourInitFailed(e: std::io::Error) { display("p2p behaviour init failed: {}", e) }
        NoiseInitFailed(e: libp2p::noise::Error) { display("p2p noise init failed: {}", e) }
        IpParseFailed(e: libp2p::multiaddr::Error) { display("p2p ip address failed: {}", e) }
        SwarmListenFailed(e: libp2p::TransportError<std::io::Error>, port: u16) { display("p2p listen failed on port {}: {}", port, e) }
        IoError(e: io::Error) { display("io error: {}", e) }
        StdError(e: String) { display("io error: {}", e) }
        TonicTransportError(e: tonic::transport::Error) { display("tonic transport error: {}", e) }
        SwarmOperationFailed(e: SendError<CommandToSwarm>) { display("swarm operation failed: {}", e) }
        SendReceiverFailed { display("sending receiver failed") }
        ChannelReceiveError(e: RecvError) { display("channel receive failed: {}", e) }
        KademliaStoreError(e: store::Error) { display("kademlia store error: {}", e) }
        InjectorError(e: String) { display("injector error: {}", e) }
        InvalidResponseChannel(e: QueryId) { display("invalid response channel for query id {:?}", e) }
        InvalidResponseChannelForRequest(e: RequestId) { display("invalid response channel for request id {:?}", e) }
        MissingInstruction { display("no instruction provided") }
        SendingResultFailed { display("sending result over channel failed") }
        MutexIsEmpty { display("mutex not initialized correctly and is empty when unwrapping") }
        GrpcClientIsEmpty { display("grpc client is not connected") }
        SettingsAddressesAreEmpty { display("settings addresses should have at least 1 address") }
        KeeperClientConnectionError { display("keeper client failed to connect") }
        JoinError(e: JoinError) { display("join error: {}", e) }
        InvalidTonicMetadataValue(e: InvalidMetadataValue) { display("invalid metadata: {}", e) }
        FailedTonicRequest(e: tonic::Status) { display("failed tonic request: {}", e) }
        MutexIsNotMutable { display("mutex not initialized correctly and is not mutable") }
        FailedConvertingFromUtf8(e: FromUtf8Error) { display("failed converting from utf-8: {}", e) }
        Utf8Error { display("failed converting from utf-8") }
        SystemTimeError(e: SystemTimeError) { display("system time error: {}", e) }
        InvalidSqlRow(e: Vec<SqlValue>) { display("invalid sql row: {:?}", e) }
        InvalidSql { display("invalid sql") }
        NoProvidersFound { display("no providers found") }
        InvalidRecordName { display("invalid record name") }
        AsyncExecutionFailed { display("async execution failed") }
        InvalidSwarmInstruction(e: CommandToSwarm) { display("invalid swarm instruction: {:?}", e) }
        InvalidNonZeroUsize { display("invalid non-zero usize ") }
        InvalidPeerId(e: ParseError) { display("invalid peer id: {}", e) }
        SwarmReqResSendResponseError { display("swarm request response send response error") }
        InsufficientReputationToStake { display("insufficient reputation to stake") }
        InsufficientReputationToUnstake { display("insufficient reputation to unstake") }
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

impl From<SendError<CommandToSwarm>> for Error {
    fn from(e: SendError<CommandToSwarm>) -> Self {
        ErrorKind::SwarmOperationFailed(e).into()
    }
}

impl From<result::Result<Vec<u8>, Error>> for Error {
    fn from(_: result::Result<Vec<u8>, Error>) -> Self {
        ErrorKind::SendingResultFailed.into()
    }
}

impl From<result::Result<(), Error>> for Error {
    fn from(_: result::Result<(), Error>) -> Self {
        ErrorKind::SendingResultFailed.into()
    }
}

impl From<RecvError> for Error {
    fn from(e: RecvError) -> Self {
        ErrorKind::ChannelReceiveError(e).into()
    }
}

impl From<oneshot::Receiver<std::result::Result<String, Error>>> for Error {
    fn from(_: oneshot::Receiver<std::result::Result<String, Error>>) -> Self {
        ErrorKind::SendReceiverFailed.into()
    }
}

impl From<oneshot::Receiver<std::result::Result<(), Error>>> for Error {
    fn from(_: oneshot::Receiver<std::result::Result<(), Error>>) -> Self {
        ErrorKind::SendReceiverFailed.into()
    }
}

impl From<store::Error> for Error {
    fn from(e: store::Error) -> Self {
        ErrorKind::KademliaStoreError(e).into()
    }
}

impl From<oneshot::Receiver<result::Result<Bytes, Error>>> for Error {
    fn from(_: oneshot::Receiver<result::Result<Bytes, Error>>) -> Self {
        ErrorKind::SendReceiverFailed.into()
    }
}

impl From<oneshot::Receiver<result::Result<HashSet<PeerId>, Error>>> for Error {
    fn from(_: oneshot::Receiver<result::Result<HashSet<PeerId>, Error>>) -> Self {
        ErrorKind::SendReceiverFailed.into()
    }
}

impl From<runtime_injector::InjectError> for Error {
    fn from(e: runtime_injector::InjectError) -> Self {
        match e {
            runtime_injector::InjectError::ActivationFailed {
                service_info,
                inner,
            } => ErrorKind::InjectorError(format!(
                "injector error for service {}: {:?}",
                service_info.name(),
                inner
            ))
            .into(),
            _ => ErrorKind::InjectorError("unknown error".to_string()).into(),
        }
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

impl From<libp2p_identity::DecodingError> for Error {
    fn from(e: libp2p_identity::DecodingError) -> Self {
        ErrorKind::KeypairBase64DecodingError(e).into()
    }
}

impl From<result::Result<HashSet<PeerId>, Error>> for Error {
    fn from(_: result::Result<HashSet<PeerId>, Error>) -> Self {
        ErrorKind::SendingResultFailed.into()
    }
}

impl From<result::Result<Vec<PeerId>, Error>> for Error {
    fn from(_: result::Result<Vec<PeerId>, Error>) -> Self {
        ErrorKind::SendingResultFailed.into()
    }
}

impl From<oneshot::Receiver<std::result::Result<Vec<PeerId>, Error>>> for Error {
    fn from(_: oneshot::Receiver<std::result::Result<Vec<PeerId>, Error>>) -> Self {
        ErrorKind::SendReceiverFailed.into()
    }
}

impl From<oneshot::Receiver<result::Result<QueryGetResponse, Error>>> for Error {
    fn from(_: oneshot::Receiver<result::Result<QueryGetResponse, Error>>) -> Self {
        ErrorKind::SendReceiverFailed.into()
    }
}

impl From<oneshot::Receiver<result::Result<VerificationResponse, Error>>> for Error {
    fn from(_: oneshot::Receiver<result::Result<VerificationResponse, Error>>) -> Self {
        ErrorKind::SendReceiverFailed.into()
    }
}

impl From<result::Result<QueryGetResponse, Error>> for Error {
    fn from(_: result::Result<QueryGetResponse, Error>) -> Self {
        ErrorKind::SendingResultFailed.into()
    }
}

impl From<result::Result<String, Error>> for Error {
    fn from(_: result::Result<String, Error>) -> Self {
        ErrorKind::SendingResultFailed.into()
    }
}
