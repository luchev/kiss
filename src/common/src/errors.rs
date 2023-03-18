use std::{error::Error as StdError, process::exit};

use config::ConfigError;
use error_chain::{error_chain, ExitCode};
use log::error;

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
        UnknownError {
            description("unknown error"),
            display("unknown error"),
        }
        DockerConnectionFailed(e: String) {
            description("could not connect to docker unix socket"),
            display("could not connect to docker unix socket: {}", e),
        }
        LocalStorageFail(e: object_store::Error) {
            description("local storage failure"),
            display("local storage failure: {}", e),
        }
        ConfigErr(e: ConfigError) {
            description("loading config failed"),
            display("loading config failed: {}", e),
        }
        SettingsDependencyFail {
            description(""),
            display(""),
        }
        SettingsParseError(e: String) {
            description(""),
            display(""),
        }
        StoragePutFailed(e: object_store::Error) {
            description("storing file failed"),
            display("storing file failed: {}", e),
        }
        StorageGetFailed(e: object_store::Error) {
            description("retrieving file failed"),
            display("retrieving file failed: {}", e),
        }
        GrpcServerStartFailed(e: tonic::transport::Error) {
            description("grpc server failed to start"),
            display("grpc server failed to start: {}", e.source().map_or("unknown transport error".to_string(), |e| e.to_string())),
        }
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
