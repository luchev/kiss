use crate::util::consts;
use crate::util::grpc::immudb_grpc::{
    immu_service_client::ImmuServiceClient, sql_value::Value, CreateDatabaseRequest, Database,
    KeyRequest, KeyValue, LoginRequest, NamedParam, NewTxRequest, NewTxResponse, SetRequest,
    SqlExecRequest, SqlQueryRequest, SqlValue, TxMetadata,
};
use crate::util::grpc::immudb_grpc::{OpenSessionRequest, TxMode};
use crate::util::types::VerificationClaim;
use crate::util::{
    types::{Bytes, Contract},
    Er, ErrorKind, Res,
};
use async_std::task::block_on;
use async_trait::async_trait;
use futures::TryFutureExt;
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
    async fn use_database(&mut self, name: String) -> Res<()>;
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
    async fn sql_execute_tx(
        &mut self,
        query: String,
        params: Vec<NamedParam>,
        session_id: String,
        transaction_id: String,
    ) -> Res<()>;
    async fn query_execute_tx(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
        session_id: String,
        transaction_id: String,
    ) -> Res<Vec<Vec<SqlValue>>>;
    async fn query_execute(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
    ) -> Res<Vec<Vec<SqlValue>>>;
    async fn get_contract(&mut self, contract_uuid: String) -> Res<Contract>;
    async fn get_all_contracts(&mut self) -> Res<Vec<Contract>>;
    async fn get_contracts(&mut self, file_uuid: String) -> Res<Vec<Contract>>;
    async fn get_reputation(&mut self, peer_id: PeerId) -> Res<i64>;
    async fn get_staked(&mut self, peer_id: PeerId) -> Res<i64>;
    async fn increase_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()>;
    async fn decrease_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()>;
    async fn stake_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()>;
    async fn unstake_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()>;
    async fn get_previous_verified(&mut self, contract_uuid: String)
        -> Res<Vec<VerificationClaim>>;
    async fn create_verified_claim(
        &mut self,
        contract_uuid: String,
        verified_by_id: PeerId,
        succeeded: bool,
    ) -> Res<()>;
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
        info!("created database");
        Ok(())
    }

    async fn use_database(&mut self, name: String) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            Database {
                database_name: name,
            },
        );
        let response = client.use_database(request).await?.into_inner();
        self.token = response.token;
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

    async fn sql_execute_tx(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
        session_id: String,
        transaction_id: String,
    ) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        map.insert("sessionid", format!("{}", session_id).parse()?);
        map.insert("transactionid", format!("{}", transaction_id).parse()?);
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

    async fn query_execute_tx(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
        session_id: String,
        transaction_id: String,
    ) -> Res<Vec<Vec<SqlValue>>> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        map.insert("sessionid", format!("{}", session_id).parse()?);
        map.insert("transactionid", format!("{}", transaction_id).parse()?);
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

    async fn get_reputation(&mut self, peer_id: PeerId) -> Res<i64> {
        let sql = "SELECT reputation FROM reputation WHERE peer_id = @peer_id;".to_string();
        let params: Vec<NamedParam> = vec![NamedParam {
            name: "peer_id".to_string(),
            value: Some(SqlValue {
                value: Some(Value::S(peer_id.to_base58())),
            }),
        }];

        let response = self.query_execute(sql, params).await?;
        let row = response.first();
        match row {
            Some(row) => Ok(match row.get(0).as_ref() {
                Some(SqlValue {
                    value: Some(Value::N(x)),
                }) => x.to_owned(),
                _ => 0,
            }),
            None => Ok(0),
        }
    }

    async fn get_staked(&mut self, peer_id: PeerId) -> Res<i64> {
        let sql = "SELECT staked FROM reputation WHERE peer_id = @peer_id;".to_string();
        let params: Vec<NamedParam> = vec![NamedParam {
            name: "peer_id".to_string(),
            value: Some(SqlValue {
                value: Some(Value::S(peer_id.to_base58())),
            }),
        }];

        let response = self.query_execute(sql, params).await?;
        let row = response.first().ok_or(ErrorKind::InvalidSql)?;
        match row.get(0).as_ref() {
            Some(SqlValue {
                value: Some(Value::N(x)),
            }) => Ok(x.to_owned()),
            _ => Ok(0),
        }
    }

    async fn increase_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()> {
        let (session_id, transaction_id) = {
            let mut client = self.client.lock().await;
            let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

            let mut map = MetadataMap::new();
            map.insert("authorization", format!("Bearer {}", self.token).parse()?);
            let request = tonic::Request::from_parts(
                map,
                Extensions::default(),
                OpenSessionRequest {
                    username: "immudb".as_bytes().to_vec(),
                    password: "immudb".as_bytes().to_vec(),
                    database_name: consts::DATABASE_NAME.to_string(),
                },
            );
            let session_id = client.open_session(request).await?.into_inner().session_id;

            let mut map = MetadataMap::new();
            map.insert("authorization", format!("Bearer {}", self.token).parse()?);
            map.insert("sessionid", format!("{}", session_id).parse()?);
            let request = tonic::Request::from_parts(
                map,
                Extensions::default(),
                NewTxRequest {
                    mode: TxMode::ReadWrite as i32,
                },
            );
            let transaction_id = client.new_tx(request).await?.into_inner().transaction_id;

            (session_id, transaction_id)
        };

        let sql = "SELECT * FROM reputation WHERE peer_id = @peer_id;".to_string();
        let params: Vec<NamedParam> = vec![NamedParam {
            name: "peer_id".to_string(),
            value: Some(SqlValue {
                value: Some(Value::S(peer_id.to_base58())),
            }),
        }];

        let response = self
            .query_execute_tx(sql, params, session_id.clone(), transaction_id.clone())
            .await?;
        let row = response.first();
        let reputation = match row {
            Some(row) => match row.get(1).as_ref() {
                Some(SqlValue {
                    value: Some(Value::N(x)),
                }) => x.to_owned(),
                _ => 0,
            },
            None => 0,
        };

        let staked = match row {
            Some(row) => match row.get(2).as_ref() {
                Some(SqlValue {
                    value: Some(Value::N(x)),
                }) => x.to_owned(),
                _ => 0,
            },
            None => 0,
        };

        let sql = "
            UPSERT
            INTO reputation(peer_id, reputation, staked)
            VALUES (@peer_id, @reputation, @staked)"
            .to_string();

        let params: Vec<NamedParam> = vec![
            NamedParam {
                name: "peer_id".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(peer_id.to_base58())),
                }),
            },
            NamedParam {
                name: "reputation".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(reputation + amount)),
                }),
            },
            NamedParam {
                name: "staked".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(staked)),
                }),
            },
        ];

        let _res = self
            .sql_execute_tx(sql, params, session_id.clone(), transaction_id.clone())
            .await;

        let mut client = self.client.lock().await;
        let client = client.as_mut().ok_or(ErrorKind::MutexIsNotMutable)?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        map.insert("sessionid", format!("{}", session_id).parse()?);
        map.insert("transactionid", format!("{}", transaction_id).parse()?);
        let request = tonic::Request::from_parts(map, Extensions::default(), ());
        client.commit(request).await?;

        let mut map = MetadataMap::new();
        map.insert("authorization", format!("Bearer {}", self.token).parse()?);
        map.insert("sessionid", format!("{}", session_id).parse()?);
        let request = tonic::Request::from_parts(map, Extensions::default(), ());
        client.close_session(request).await?;
        Ok(())
    }

    async fn decrease_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()> {
        self.increase_reputation(peer_id, -amount).await
    }

    async fn stake_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()> {
        let rep = self.get_reputation(peer_id).await?;
        if rep < amount {
            return Err(ErrorKind::InsufficientReputationToStake.into());
        }

        let sql = "
                UPSERT
                INTO reputation(peer_id, reputation, staked)
                VALUES (@peer_id, @rep, @amount)"
            .to_string();

        let params: Vec<NamedParam> = vec![
            NamedParam {
                name: "peer_id".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(peer_id.to_base58())),
                }),
            },
            NamedParam {
                name: "rep".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(rep - amount)),
                }),
            },
            NamedParam {
                name: "staked".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(rep + amount)),
                }),
            },
        ];

        self.sql_execute(sql, params).await
    }

    async fn unstake_reputation(&mut self, peer_id: PeerId, amount: i64) -> Res<()> {
        let rep = self.get_reputation(peer_id).await?;
        let staked = self.get_staked(peer_id).await?;
        if staked < amount {
            return Err(ErrorKind::InsufficientReputationToUnstake.into());
        }

        let sql = "
                UPSERT
                INTO reputation(peer_id, reputation, staked)
                VALUES (@peer_id, @rep, @staked)"
            .to_string();

        let params: Vec<NamedParam> = vec![
            NamedParam {
                name: "peer_id".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(peer_id.to_base58())),
                }),
            },
            NamedParam {
                name: "rep".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(rep + amount)),
                }),
            },
            NamedParam {
                name: "staked".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(staked - amount)),
                }),
            },
        ];

        self.sql_execute(sql, params).await
    }

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
        let sql = "SELECT * FROM contracts;".to_string();

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

    async fn get_previous_verified(
        &mut self,
        contract_uuid: String,
    ) -> Res<Vec<VerificationClaim>> {
        let sql = "SELECT * FROM verifications WHERE contract_uuid = @contract_uuid;".to_string();
        let params: Vec<NamedParam> = vec![NamedParam {
            name: "contract_uuid".to_string(),
            value: Some(SqlValue {
                value: Some(Value::S(contract_uuid)),
            }),
        }];

        let response = self.query_execute(sql, params).await?;
        let claims: Res<Vec<_>> = response
            .into_iter()
            .map(map_row_to_verification_claim)
            .collect();
        Ok(claims?)
    }

    async fn create_verified_claim(
        &mut self,
        contract_uuid: String,
        verified_by_id: PeerId,
        succeeded: bool,
    ) -> Res<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
        let params: Vec<NamedParam> = vec![
            NamedParam {
                name: "contract_uuid".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(contract_uuid)),
                }),
            },
            NamedParam {
                name: "verified_by_id".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(verified_by_id.to_base58())),
                }),
            },
            NamedParam {
                name: "verification_time".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::N(now)),
                }),
            },
            NamedParam {
                name: "succeeded".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::B(succeeded)),
                }),
            },
        ];

        let sql = "UPSERT
                INTO verifications(contract_uuid, verified_by_id, verification_time, succeeded)
                VALUES (@contract_uuid, @verified_by_id, @verification_time, @succeeded);"
            .to_string();

        let _response = self.sql_execute(sql, params).await?;
        Ok(())
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
            }) => PeerId::from_str(x).map_err(ErrorKind::InvalidPeerId)?,
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

fn map_row_to_verification_claim(row: Vec<SqlValue>) -> Res<VerificationClaim> {
    Ok(VerificationClaim {
        contract_uuid: match row.get(0).as_ref() {
            Some(SqlValue {
                value: Some(Value::S(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        verified_by_id: match row.get(1).as_ref() {
            Some(SqlValue {
                value: Some(Value::S(x)),
            }) => PeerId::from_str(x).map_err(ErrorKind::InvalidPeerId)?,
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        verification_time: match row.get(2).as_ref() {
            Some(SqlValue {
                value: Some(Value::N(x)),
            }) => x.to_owned(),
            _ => Err(ErrorKind::InvalidSqlRow(row.clone()))?,
        },
        succeeded: match row.get(3).as_ref() {
            Some(SqlValue {
                value: Some(Value::B(x)),
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
        let ledger = block_on(async { handle.spawn(init_database(ledger)).await })
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

async fn init_database(ledger: ImmuLedger) -> Res<ImmuLedger> {
    create_database(ledger)
        .and_then(use_database)
        .and_then(create_contract_table)
        .and_then(create_reputations_table)
        .and_then(create_verifications_table)
        .await
}

async fn create_database(mut ledger: ImmuLedger) -> Res<ImmuLedger> {
    ledger
        .create_database(consts::DATABASE_NAME.to_string())
        .await?;
    Ok(ledger)
}

async fn use_database(mut ledger: ImmuLedger) -> Res<ImmuLedger> {
    ledger
        .use_database(consts::DATABASE_NAME.to_string())
        .await?;
    Ok(ledger)
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

async fn create_reputations_table(mut ledger: ImmuLedger) -> Res<ImmuLedger> {
    let query = "CREATE TABLE IF NOT EXISTS reputation (
            peer_id         VARCHAR[53],
            reputation      INTEGER,
            staked          INTEGER,
            PRIMARY KEY (peer_id)
        );"
    .to_string();

    ledger.sql_execute(query, vec![]).await?;
    Ok(ledger)
}

async fn create_verifications_table(mut ledger: ImmuLedger) -> Res<ImmuLedger> {
    let query = "CREATE TABLE IF NOT EXISTS verifications (
            contract_uuid     VARCHAR[36],
            verified_by_id    VARCHAR[53],
            verification_time INTEGER,
            succeeded         BOOLEAN,
            PRIMARY KEY (contract_uuid, verified_by_id)
        );"
    .to_string();

    ledger.sql_execute(query, vec![]).await?;
    Ok(ledger)
}
