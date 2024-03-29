use crate::util::grpc::immudb_grpc::{
    immu_service_client::ImmuServiceClient, sql_value::Value, CreateDatabaseRequest, KeyRequest,
    KeyValue, LoginRequest, NamedParam, SetRequest, SqlExecRequest, SqlQueryRequest, SqlValue,
};
use crate::util::{
    types::{Bytes, Contract},
    Er, ErrorKind, Res,
};
use async_std::task::block_on;
use async_trait::async_trait;
use libp2p_identity::PeerId;
use log::info;
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};
use std::str::FromStr;
use std::{
    net::SocketAddr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{runtime::Handle, sync::Mutex};
use tonic::{metadata::MetadataMap, transport::Channel, Extensions};
use uuid::Uuid;

use crate::settings::{ISettings, Ledger};

#[async_trait]
pub trait ILedger: Service {
    async fn set(&mut self, key: String, value: Bytes) -> Res<()>;
    async fn get(&mut self, key: String) -> Res<String>;
    async fn create_database(&mut self, name: String) -> Res<()>;
    async fn create_contract(
        &mut self,
        peer_id: PeerId,
        file_uuid: Uuid,
        file_hash: String,
        ttl: i64,
        secret_n: Bytes,
        secret_m: Bytes,
        rows: i64,
        cols: i64,
    ) -> Res<()>;
    async fn sql_execute(&mut self, query: String, params: Vec<NamedParam>) -> Res<()>;
    async fn query_execute(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
    ) -> Res<Vec<Vec<SqlValue>>>;
    async fn get_contract(&mut self, contract_uuid: String) -> Res<Contract>;
    async fn get_all_contracts(&mut self) -> Res<Vec<Contract>>;
    async fn get_contracts(&mut self, file_uuid: String) -> Res<Vec<Contract>>;
}

#[derive(Debug)]
pub struct ImmuLedger {
    token: String,
    client: Mutex<Option<ImmuServiceClient<Channel>>>,
}

#[async_trait]
impl ILedger for ImmuLedger {
    async fn set(&mut self, key: String, value: Bytes) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            SetRequest {
                k_vs: vec![KeyValue {
                    key: key.as_bytes().to_vec(),
                    value,
                    metadata: None,
                }],
                no_wait: false,
                preconditions: vec![],
            },
        );
        let _response = client.set(request).await?;
        Ok(())
    }

    async fn get(&mut self, key: String) -> Res<String> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);

        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            KeyRequest {
                key: key.as_bytes().to_vec(),
                no_wait: false,
                at_revision: 0,
                at_tx: 0,
                since_tx: 0,
            },
        );
        let response = client.get(request).await?;
        Ok(String::from_utf8(response.into_inner().value)?)
    }

    async fn create_database(&mut self, name: String) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            CreateDatabaseRequest {
                name,
                settings: None,
                if_not_exists: true,
            },
        );
        let _response = client.create_database_v2(request).await?;
        Ok(())
    }

    async fn sql_execute(&mut self, sql: String, params: Vec<NamedParam>) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            SqlExecRequest {
                sql,
                params,
                no_wait: false,
            },
        );
        let _response = client.sql_exec(request).await?;
        Ok(())
    }

    async fn query_execute(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
    ) -> Res<Vec<Vec<SqlValue>>> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            SqlQueryRequest {
                sql,
                params,
                reuse_snapshot: false,
            },
        );
        let response = client.sql_query(request).await?;
        let result: Vec<_> = response
            .into_inner()
            .rows
            .into_iter()
            .map(|row| row.values)
            .collect();
        Ok(result)
    }

    async fn create_contract(
        &mut self,
        peer_id: PeerId,
        file_uuid: Uuid,
        file_hash: String,
        ttl: i64,
        secret_n: Bytes,
        secret_m: Bytes,
        rows: i64,
        cols: i64,
    ) -> Res<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let params: Vec<NamedParam> = vec![
            NamedParam {
                name: "contract_uuid".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(Uuid::new_v4().to_string())),
                }),
            },
            NamedParam {
                name: "peer_id".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(peer_id.to_base58())),
                }),
            },
            NamedParam {
                name: "file_uuid".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(file_uuid.to_string())),
                }),
            },
            NamedParam {
                name: "file_hash".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(file_hash)),
                }),
            },
            NamedParam {
                name: "upload_date".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(now)),
                }),
            },
            NamedParam {
                name: "ttl".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(ttl)),
                }),
            },
            NamedParam {
                name: "secret_n".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::Bs(secret_n)),
                }),
            },
            NamedParam {
                name: "secret_m".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::Bs(secret_m)),
                }),
            },
            NamedParam {
                name: "rows".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(rows)),
                }),
            },
            NamedParam {
                name: "cols".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(cols)),
                }),
            },
        ];

        let sql = "UPSERT
                INTO contracts(contract_uuid, peer_id, file_uuid, file_hash, upload_date, ttl, secret_n, secret_m, rows, cols)
                VALUES (@contract_uuid, @peer_id, @file_uuid, @file_hash, @upload_date, @ttl, @secret_n, @secret_m, @rows, @cols);"
            .to_string();

        let _response = self.sql_execute(sql, params).await?;
        Ok(())
    }

    // async fn create_contract_old(
    //     &mut self,
    //     file_uuid: Uuid,
    //     file_hash: String,
    //     ttl: i64,
    // ) -> Res<()> {
    //     let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    //     let params: Vec<NamedParam> = vec![
    //         NamedParam {
    //             name: "contract_uuid".to_string(),
    //             value: Some(SqlValue {
    //                 value: Some(Value::S(Uuid::new_v4().to_string())),
    //             }),
    //         },
    //         NamedParam {
    //             name: "file_uuid".to_string(),
    //             value: Some(SqlValue {
    //                 value: Some(Value::S(file_uuid.to_string())),
    //             }),
    //         },
    //         NamedParam {
    //             name: "file_hash".to_string(),
    //             value: Some(SqlValue {
    //                 value: Some(Value::S(file_hash)),
    //             }),
    //         },
    //         NamedParam {
    //             name: "upload_date".to_string(),
    //             value: Some(SqlValue {
    //                 value: Some(Value::N(now)),
    //             }),
    //         },
    //         NamedParam {
    //             name: "ttl".to_string(),
    //             value: Some(SqlValue {
    //                 value: Some(Value::N(ttl)),
    //             }),
    //         },
    //     ];

    //     let sql = "UPSERT
    //             INTO contracts(contract_uuid, file_uuid, file_hash, upload_date, ttl)
    //             VALUES (@contract_uuid, @file_uuid, @file_hash, @upload_date, @ttl);"
    //         .to_string();

    //     let _response = self.sql_execute(sql, params).await?;
    //     Ok(())
    // }

    async fn get_contract(&mut self, file_uuid: String) -> Res<Contract> {
        let sql = "SELECT * FROM contracts WHERE file_uuid = @file_uuid;".to_string();

        let params: Vec<NamedParam> = vec![NamedParam {
            name: "file_uuid".to_string(),
            value: Some(SqlValue {
                value: Some(Value::S(file_uuid)),
            }),
        }];
        let response = self.query_execute(sql, params).await?;
        let row = response.first().ok_or(ErrorKind::InvalidSql)?;
        map_row_to_contract(row.clone())
    }

    async fn get_all_contracts(&mut self) -> Res<Vec<Contract>> {
        let sql = "SELECT * FROM contracts LIMIT 100;".to_string();

        let response = self.query_execute(sql, vec![]).await?;
        let contracts: Res<Vec<_>> = response.into_iter().map(map_row_to_contract).collect();
        Ok(contracts?)
    }

    async fn get_contracts(&mut self, file_uuid: String) -> Res<Vec<Contract>> {
        let sql = "SELECT * FROM contracts WHERE file_uuid = @file_uuid;".to_string();
        let params: Vec<NamedParam> = vec![NamedParam {
            name: "file_uuid".to_string(),
            value: Some(SqlValue {
                value: Some(Value::S(file_uuid)),
            }),
        }];

        let response = self.query_execute(sql, params).await?;
        let contracts: Res<Vec<_>> = response.into_iter().map(map_row_to_contract).collect();
        Ok(contracts?)
    }
}

fn map_row_to_contract(row: Vec<SqlValue>) -> Res<Contract> {
    Ok(Contract {
        contract_uuid: match row.get(0).as_ref() {
            Some(SqlValue {
                value: Some(Value::S(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        peer_id: match row.get(1).as_ref() {
            Some(SqlValue {
                value: Some(Value::S(x)),
            }) => PeerId::from_str(x).map_err(|e| ErrorKind::InvalidPeerId(e))?,
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        file_uuid: match row.get(2).as_ref() {
            Some(SqlValue {
                value: Some(Value::S(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        file_hash: match row.get(3).as_ref() {
            Some(SqlValue {
                value: Some(Value::S(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        upload_date: match row.get(4).as_ref() {
            Some(SqlValue {
                value: Some(Value::N(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        ttl: match row.get(5).as_ref() {
            Some(SqlValue {
                value: Some(Value::N(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        secret_n: match row.get(6).as_ref() {
            Some(SqlValue {
                value: Some(Value::Bs(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        secret_m: match row.get(7).as_ref() {
            Some(SqlValue {
                value: Some(Value::Bs(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        rows: match row.get(8).as_ref() {
            Some(SqlValue {
                value: Some(Value::N(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        cols: match row.get(9).as_ref() {
            Some(SqlValue {
                value: Some(Value::N(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
    })
}

pub struct LedgerProvider;
impl ServiceFactory<()> for LedgerProvider {
    type Result = Mutex<ImmuLedger>;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let settings = injector.get::<Svc<dyn ISettings>>()?.ledger();
        let ledger = match settings {
            Ledger::Immudb {
                username,
                password,
                address,
            } => {
                let handle = Handle::current();
                let (client, token) = match block_on(async {
                    handle.spawn(login(address, username, password)).await
                }) {
                    Ok(Ok(x)) => x,
                    Ok(Err(e)) => Err(InjectError::ActivationFailed {
                        service_info: ServiceInfo::of::<ImmuLedger>(),
                        inner: Box::<Er>::new(e),
                    })?,
                    Err(e) => Err(InjectError::ActivationFailed {
                        service_info: ServiceInfo::of::<ImmuLedger>(),
                        inner: Box::<Er>::new(ErrorKind::JoinError(e).into()),
                    })?,
                };

                ImmuLedger {
                    token,
                    client: Mutex::new(Some(client)),
                }
            }
        };

        let handle = Handle::current();
        let ledger = block_on(async { handle.spawn(create_contract_table(ledger)).await })
            .map_err(|e| InjectError::ActivationFailed {
                service_info: ServiceInfo::of::<ImmuLedger>(),
                inner: Box::<Er>::new(ErrorKind::JoinError(e).into()),
            })?
            .map_err(|e| InjectError::ActivationFailed {
                service_info: ServiceInfo::of::<ImmuLedger>(),
                inner: Box::<Er>::new(e),
            })?;

        Ok(Mutex::new(ledger))
    }
}

interface! {
    dyn ILedger = [
        ImmuLedger,
    ]
}

async fn login(
    address: SocketAddr,
    username: String,
    password: String,
) -> Res<(ImmuServiceClient<Channel>, String)> {
    let mut client = Some(ImmuServiceClient::connect(format!("http://{}", address)).await?);

    let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;
    let request = tonic::Request::new(LoginRequest {
        user: username.as_bytes().to_vec(),
        password: password.as_bytes().to_vec(),
    });
    let response = client.login(request).await?;

    let token = response.into_inner().token;
    info!("logged into immudb");
    Ok((client.to_owned(), token))
}

async fn create_contract_table(mut ledger: ImmuLedger) -> Res<ImmuLedger> {
    let query = "CREATE TABLE IF NOT EXISTS contracts (
            contract_uuid   VARCHAR[36],
            peer_id         VARCHAR[53],
            file_uuid       VARCHAR[36],
            file_hash       VARCHAR[1024],
            upload_date     INTEGER,
            ttl             INTEGER,
            secret_n        BLOB,
            secret_m        BLOB,
            rows            INTEGER,
            cols            INTEGER,
            PRIMARY KEY (file_uuid)
        );"
    .to_string();

    ledger.sql_execute(query, vec![]).await?;
    Ok(ledger)
}
