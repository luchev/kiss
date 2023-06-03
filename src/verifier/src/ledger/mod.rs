use crate::{
    immudb_grpc::{
        immu_service_client::ImmuServiceClient, sql_value::Value, CreateDatabaseRequest,
        KeyRequest, KeyValue, LoginRequest, NamedParam, SetRequest, SqlExecRequest,
        SqlQueryRequest, SqlValue,
    },
    settings::{ISettings, Ledger},
    types::{Bytes, Contract},
};
use async_std::task::block_on;
use async_trait::async_trait;
use common::Res;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::{time::{SystemTime, UNIX_EPOCH}, net::SocketAddr};
use tokio::{runtime::Handle, sync::Mutex};
use tonic::{metadata::MetadataMap, transport::Channel, Extensions};
use uuid::Uuid;

#[async_trait]
pub trait ILedger: Service {
    async fn set(&mut self, key: String, value: Bytes) -> Res<()>;
    async fn get(&mut self, key: String) -> Res<String>;
    async fn create_database(&mut self, name: String) -> Res<()>;
    async fn create_contract(&mut self, file_hash: String, ttl: i64) -> Res<String>;
    async fn sql_execute(&mut self, query: String, params: Vec<NamedParam>) -> Res<()>;
    async fn query_execute(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
    ) -> Res<Vec<Vec<SqlValue>>>;
    async fn get_contract(&mut self, contract_uuid: String) -> Res<Contract>;
    async fn get_contracts(&mut self) -> Res<Vec<Contract>>;
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
        let client = client.as_mut().unwrap();

        let mut map = MetadataMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            SetRequest {
                k_vs: vec![KeyValue {
                    key: key.as_bytes().to_vec(),
                    value: value,
                    metadata: None,
                }],
                no_wait: false,
                preconditions: vec![],
            },
        );
        let _response = client.set(request).await.unwrap();
        Ok(())
    }

    async fn get(&mut self, key: String) -> Res<String> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();

        let mut map = MetadataMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );

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
        let response = client.get(request).await.unwrap();
        Ok(String::from_utf8(response.into_inner().value).unwrap())
    }

    async fn create_database(&mut self, name: String) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();

        let mut map = MetadataMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            CreateDatabaseRequest {
                name,
                settings: None,
                if_not_exists: true,
            },
        );
        let _response = client.create_database_v2(request).await.unwrap();
        Ok(())
    }

    async fn sql_execute(&mut self, sql: String, params: Vec<NamedParam>) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();

        let mut map = MetadataMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            SqlExecRequest {
                sql,
                params,
                no_wait: false,
            },
        );
        let _response = client.sql_exec(request).await.unwrap();
        Ok(())
    }

    async fn query_execute(
        &mut self,
        sql: String,
        params: Vec<NamedParam>,
    ) -> Res<Vec<Vec<SqlValue>>> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();

        let mut map = MetadataMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            SqlQueryRequest {
                sql,
                params,
                reuse_snapshot: false,
            },
        );
        let response = client.sql_query(request).await.unwrap();
        let result: Vec<_> = response
            .into_inner()
            .rows
            .into_iter()
            .map(|row| row.values)
            .collect();
        Ok(result)
    }

    async fn create_contract(&mut self, file_hash: String, ttl: i64) -> Res<String> {
        let file_uuid = Uuid::new_v4();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let params: Vec<NamedParam> = vec![
            NamedParam {
                name: "contract_uuid".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(Uuid::new_v4().to_string())),
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
        ];

        let sql = "UPSERT
                INTO contracts(contract_uuid, file_uuid, file_hash, upload_date, ttl)
                VALUES (@contract_uuid, @file_uuid, @file_hash, @upload_date, @ttl);"
            .to_string();

        let _response = self.sql_execute(sql, params).await.unwrap();
        Ok(file_uuid.to_string())
    }

    async fn get_contract(&mut self, file_uuid: String) -> Res<Contract> {
        let sql = "SELECT * FROM contracts WHERE file_uuid = @file_uuid;".to_string();
        info!("{:?}", sql);

        let params: Vec<NamedParam> = vec![
            NamedParam {
                name: "file_uuid".to_string(),
                value: Some(SqlValue {
                    value: Some(Value::S(file_uuid)),
                }),
            },
        ];
        let response = self.query_execute(sql, params).await.unwrap();
        let contracts: Vec<_> = response
            .into_iter()
            .map(|row| Contract {
                contract_uuid: match row[0].value.as_ref().unwrap() {
                    Value::S(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                file_uuid: match row[1].value.as_ref().unwrap() {
                    Value::S(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                file_hash: match row[2].value.as_ref().unwrap() {
                    Value::S(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                upload_date: match row[3].value.as_ref().unwrap() {
                    Value::N(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                ttl: match row[4].value.as_ref().unwrap() {
                    Value::N(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
            })
            .collect();
        Ok(contracts.into_iter().next().unwrap_or_default())
    }

    async fn get_contracts(&mut self) -> Res<Vec<Contract>> {
        let sql = "SELECT * FROM contracts;".to_string();

        let response = self.query_execute(sql, vec![]).await.unwrap();
        let contracts: Vec<_> = response
            .into_iter()
            .map(|row| Contract {
                contract_uuid: match row[0].value.as_ref().unwrap() {
                    Value::S(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                file_uuid: match row[1].value.as_ref().unwrap() {
                    Value::S(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                file_hash: match row[2].value.as_ref().unwrap() {
                    Value::S(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                upload_date: match row[3].value.as_ref().unwrap() {
                    Value::N(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
                ttl: match row[4].value.as_ref().unwrap() {
                    Value::N(x) => x.clone(),
                    _ => panic!("unexpected type received from immudb"),
                },
            })
            .collect();
        Ok(contracts)
    }
}

pub struct LedgerProvider;
impl ServiceFactory<()> for LedgerProvider {
    type Result = Mutex<ImmuLedger>;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let settings = injector.get::<Svc<dyn ISettings>>().unwrap().ledger();
        let result = match settings {
            Ledger::Immudb {
                username,
                password,
                address,
            } => {
                let handle = Handle::current();
                let (client, token) = block_on(async {
                    handle
                        .spawn(login(address, username, password))
                        .await
                        .unwrap()
                });

                ImmuLedger {
                    token: token,
                    client: Mutex::new(Some(client)),
                }
            }
        };

        let handle = Handle::current();
        let ledger =
            block_on(async { handle.spawn(create_contract_table(result)).await.unwrap() }).unwrap();

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
) -> (ImmuServiceClient<Channel>, String) {
    let mut client = Some(
        ImmuServiceClient::connect(format!("http://{}", address))
            .await
            .expect("failed to connect to immudb"),
    );

    let client = client.as_mut().expect("invalid immudb client");
    let request = tonic::Request::new(LoginRequest {
        user: username.as_bytes().to_vec(),
        password: password.as_bytes().to_vec(),
    });
    let response = client
        .login(request)
        .await
        .expect("failed to login to immudb");

    let token = response.into_inner().token;
    info!("logged into immudb");
    (client.to_owned(), token)
}

async fn create_contract_table(mut ledger: ImmuLedger) -> Res<ImmuLedger> {
    let query = "CREATE TABLE IF NOT EXISTS contracts (
            contract_uuid   VARCHAR[36],
            file_uuid       VARCHAR[36],
            file_hash       VARCHAR[1024],
            upload_date     INTEGER,
            ttl             INTEGER,
            PRIMARY KEY (file_uuid)
        );"
    .to_string();

    let _response = ledger.sql_execute(query, vec![]).await.unwrap();
    Ok(ledger)
}
