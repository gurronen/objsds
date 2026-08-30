//! Isolated Node-API adapter for [`objsds`].
//!
//! The binding intentionally exchanges application values as JSON strings.
//! This keeps JavaScript conversion policy in the TypeScript facade and keeps
//! Node-specific types out of the core crates.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use napi::bindgen_prelude::AsyncTask;
use napi::{Env, Error as NapiError, Result as NapiResult, Status, Task};
use napi_derive::napi;
use objsds::s3::{Credentials, S3Store};
use objsds::{Error, InsertIfAbsent, Log, Map, Objsds};
use objsds_store::ObjectStore;
use objsds_store_memory::MemoryStore;
use serde_json::{Value, json};

#[derive(Clone)]
enum Client {
    Memory(Objsds<MemoryStore>),
    S3(Objsds<S3Store>),
}

trait BoundMap: Send + Sync {
    fn get_json(&self, key: &str) -> NapiResult<String>;
    fn entries_json(&self) -> NapiResult<String>;
    fn insert_json(&self, key: String, value: Value) -> NapiResult<String>;
    fn insert_if_absent_json(&self, key: String, value: Value) -> NapiResult<String>;
    fn remove_json(&self, key: &str) -> NapiResult<String>;
}

impl<S> BoundMap for Map<S, Value>
where
    S: ObjectStore + Send + Sync,
    S::Error: Display,
{
    fn get_json(&self, key: &str) -> NapiResult<String> {
        optional_json(self.get(key).map_err(operation_error)?)
    }

    fn entries_json(&self) -> NapiResult<String> {
        json_string(&self.entries().map_err(operation_error)?)
    }

    fn insert_json(&self, key: String, value: Value) -> NapiResult<String> {
        let version = self.insert(key, value).map_err(operation_error)?;
        json_string(version.as_str())
    }

    fn insert_if_absent_json(&self, key: String, value: Value) -> NapiResult<String> {
        let result = self.insert_if_absent(key, value).map_err(operation_error)?;
        let result = match result {
            InsertIfAbsent::Inserted(version) => {
                json!({ "inserted": true, "version": version.as_str() })
            }
            InsertIfAbsent::Occupied(value) => json!({ "inserted": false, "value": value }),
        };
        json_string(&result)
    }

    fn remove_json(&self, key: &str) -> NapiResult<String> {
        optional_json(self.remove(key).map_err(operation_error)?)
    }
}

trait BoundLog: Send + Sync {
    fn append_json(&self, value: Value) -> NapiResult<String>;
    fn get_json(&self, id: objsds::LogId) -> NapiResult<String>;
    fn records_json(&self) -> NapiResult<String>;
    fn records_after_json(&self, id: objsds::LogId) -> NapiResult<String>;
}

impl<S> BoundLog for Log<S, Value>
where
    S: ObjectStore + Send + Sync,
    S::Error: Display,
{
    fn append_json(&self, value: Value) -> NapiResult<String> {
        json_string(&self.append(value).map_err(operation_error)?.to_string())
    }

    fn get_json(&self, id: objsds::LogId) -> NapiResult<String> {
        optional_json(self.get(id).map_err(operation_error)?)
    }

    fn records_json(&self) -> NapiResult<String> {
        json_string(&self.records().map_err(operation_error)?)
    }

    fn records_after_json(&self, id: objsds::LogId) -> NapiResult<String> {
        json_string(&self.records_after(id).map_err(operation_error)?)
    }
}

type StoredMap = Arc<dyn BoundMap>;
type StoredLog = Arc<dyn BoundLog>;

struct State {
    client: Client,
    next_handle: AtomicU32,
    maps: Mutex<HashMap<u32, StoredMap>>,
    logs: Mutex<HashMap<u32, StoredLog>>,
}

impl State {
    fn new(client: Client) -> Self {
        Self {
            client,
            next_handle: AtomicU32::new(1),
            maps: Mutex::new(HashMap::new()),
            logs: Mutex::new(HashMap::new()),
        }
    }

    fn handle(&self) -> u32 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }
}

/// A native client shared by the TypeScript facade.
#[napi]
pub struct NativeClient {
    state: Arc<State>,
}

/// Constructs an in-memory native client.
#[napi]
pub fn memory_client(namespace: String) -> NapiResult<NativeClient> {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace(namespace)
        .build()
        .map_err(configuration_error)?;
    Ok(NativeClient {
        state: Arc::new(State::new(Client::Memory(client))),
    })
}

/// Constructs an S3-compatible native client.
#[napi]
#[allow(clippy::too_many_arguments)]
pub fn s3_client(
    namespace: String,
    bucket: String,
    region: String,
    endpoint: Option<String>,
    path_style: bool,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
) -> NapiResult<NativeClient> {
    let credentials = match (access_key_id, secret_access_key) {
        (None, None) => Credentials::Default,
        (Some(access_key_id), Some(secret_access_key)) => {
            let credentials = Credentials::new(access_key_id, secret_access_key);
            match session_token {
                Some(token) => credentials.with_session_token(token),
                None => credentials,
            }
        }
        _ => {
            return Err(binding_error(
                "ERR_OBJSDS_INVALID_CONFIGURATION",
                "accessKeyId and secretAccessKey must be supplied together",
                json!({}),
            ));
        }
    };

    let mut store = S3Store::builder()
        .bucket(bucket)
        .region(region)
        .credentials(credentials)
        .path_style(path_style);
    if let Some(endpoint) = endpoint {
        store = store.endpoint(endpoint);
    }
    let store = store.build().map_err(configuration_error)?;
    let client = Objsds::builder()
        .store(store)
        .namespace(namespace)
        .build()
        .map_err(configuration_error)?;
    Ok(NativeClient {
        state: Arc::new(State::new(Client::S3(client))),
    })
}

#[napi]
impl NativeClient {
    /// Creates a map and returns an opaque native handle encoded as JSON.
    #[napi(js_name = "mapCreate")]
    pub fn map_create(&self, name: String, schema: String) -> AsyncTask<BindingTask> {
        self.task(Operation::MapLifecycle {
            lifecycle: Lifecycle::Create,
            name,
            schema,
        })
    }

    /// Opens a map and returns an opaque native handle encoded as JSON.
    #[napi(js_name = "mapOpen")]
    pub fn map_open(&self, name: String, schema: String) -> AsyncTask<BindingTask> {
        self.task(Operation::MapLifecycle {
            lifecycle: Lifecycle::Open,
            name,
            schema,
        })
    }

    /// Opens or creates a map and returns an opaque native handle encoded as JSON.
    #[napi(js_name = "mapOpenOrCreate")]
    pub fn map_open_or_create(&self, name: String, schema: String) -> AsyncTask<BindingTask> {
        self.task(Operation::MapLifecycle {
            lifecycle: Lifecycle::OpenOrCreate,
            name,
            schema,
        })
    }

    /// Reads one map entry.
    #[napi(js_name = "mapGet")]
    pub fn map_get(&self, handle: u32, key: String) -> AsyncTask<BindingTask> {
        self.task(Operation::MapGet { handle, key })
    }

    /// Reads all map entries.
    #[napi(js_name = "mapEntries")]
    pub fn map_entries(&self, handle: u32) -> AsyncTask<BindingTask> {
        self.task(Operation::MapEntries { handle })
    }

    /// Inserts one map entry.
    #[napi(js_name = "mapInsert")]
    pub fn map_insert(
        &self,
        handle: u32,
        key: String,
        value_json: String,
    ) -> AsyncTask<BindingTask> {
        self.task(Operation::MapInsert {
            handle,
            key,
            value_json,
        })
    }

    /// Inserts one map entry when absent.
    #[napi(js_name = "mapInsertIfAbsent")]
    pub fn map_insert_if_absent(
        &self,
        handle: u32,
        key: String,
        value_json: String,
    ) -> AsyncTask<BindingTask> {
        self.task(Operation::MapInsertIfAbsent {
            handle,
            key,
            value_json,
        })
    }

    /// Removes one map entry.
    #[napi(js_name = "mapRemove")]
    pub fn map_remove(&self, handle: u32, key: String) -> AsyncTask<BindingTask> {
        self.task(Operation::MapRemove { handle, key })
    }

    /// Creates a log and returns an opaque native handle encoded as JSON.
    #[napi(js_name = "logCreate")]
    pub fn log_create(&self, name: String, schema: String) -> AsyncTask<BindingTask> {
        self.task(Operation::LogLifecycle {
            lifecycle: Lifecycle::Create,
            name,
            schema,
        })
    }

    /// Opens a log and returns an opaque native handle encoded as JSON.
    #[napi(js_name = "logOpen")]
    pub fn log_open(&self, name: String, schema: String) -> AsyncTask<BindingTask> {
        self.task(Operation::LogLifecycle {
            lifecycle: Lifecycle::Open,
            name,
            schema,
        })
    }

    /// Opens or creates a log and returns an opaque native handle encoded as JSON.
    #[napi(js_name = "logOpenOrCreate")]
    pub fn log_open_or_create(&self, name: String, schema: String) -> AsyncTask<BindingTask> {
        self.task(Operation::LogLifecycle {
            lifecycle: Lifecycle::OpenOrCreate,
            name,
            schema,
        })
    }

    /// Appends one log value.
    #[napi(js_name = "logAppend")]
    pub fn log_append(&self, handle: u32, value_json: String) -> AsyncTask<BindingTask> {
        self.task(Operation::LogAppend { handle, value_json })
    }

    /// Reads one log record.
    #[napi(js_name = "logGet")]
    pub fn log_get(&self, handle: u32, id: String) -> AsyncTask<BindingTask> {
        self.task(Operation::LogGet { handle, id })
    }

    /// Reads all log records.
    #[napi(js_name = "logRecords")]
    pub fn log_records(&self, handle: u32) -> AsyncTask<BindingTask> {
        self.task(Operation::LogRecords { handle })
    }

    /// Reads log records after an identifier.
    #[napi(js_name = "logRecordsAfter")]
    pub fn log_records_after(&self, handle: u32, id: String) -> AsyncTask<BindingTask> {
        self.task(Operation::LogRecordsAfter { handle, id })
    }

    fn task(&self, operation: Operation) -> AsyncTask<BindingTask> {
        AsyncTask::new(BindingTask {
            state: Arc::clone(&self.state),
            operation: Some(operation),
        })
    }
}

/// A libuv worker-pool task. Every objsds operation is blocking and therefore
/// must execute here rather than on JavaScript's event-loop thread.
pub struct BindingTask {
    state: Arc<State>,
    operation: Option<Operation>,
}

impl Task for BindingTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> NapiResult<Self::Output> {
        let operation = self.operation.take().ok_or_else(|| {
            NapiError::new(Status::GenericFailure, "binding task was already consumed")
        })?;
        execute(&self.state, operation)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> NapiResult<Self::JsValue> {
        Ok(output)
    }
}

#[derive(Clone, Copy)]
enum Lifecycle {
    Create,
    Open,
    OpenOrCreate,
}

enum Operation {
    MapLifecycle {
        lifecycle: Lifecycle,
        name: String,
        schema: String,
    },
    MapGet {
        handle: u32,
        key: String,
    },
    MapEntries {
        handle: u32,
    },
    MapInsert {
        handle: u32,
        key: String,
        value_json: String,
    },
    MapInsertIfAbsent {
        handle: u32,
        key: String,
        value_json: String,
    },
    MapRemove {
        handle: u32,
        key: String,
    },
    LogLifecycle {
        lifecycle: Lifecycle,
        name: String,
        schema: String,
    },
    LogAppend {
        handle: u32,
        value_json: String,
    },
    LogGet {
        handle: u32,
        id: String,
    },
    LogRecords {
        handle: u32,
    },
    LogRecordsAfter {
        handle: u32,
        id: String,
    },
}

fn execute(state: &State, operation: Operation) -> NapiResult<String> {
    match operation {
        Operation::MapLifecycle {
            lifecycle,
            name,
            schema,
        } => {
            let handle = state.handle();
            let map: StoredMap = match &state.client {
                Client::Memory(client) => Arc::new(open_map(client, lifecycle, name, schema)?),
                Client::S3(client) => Arc::new(open_map(client, lifecycle, name, schema)?),
            };
            lock(&state.maps).insert(handle, map);
            json_string(&handle)
        }
        Operation::MapGet { handle, key } => map(state, handle)?.get_json(&key),
        Operation::MapEntries { handle } => map(state, handle)?.entries_json(),
        Operation::MapInsert {
            handle,
            key,
            value_json,
        } => map(state, handle)?.insert_json(key, parse_value(&value_json)?),
        Operation::MapInsertIfAbsent {
            handle,
            key,
            value_json,
        } => map(state, handle)?.insert_if_absent_json(key, parse_value(&value_json)?),
        Operation::MapRemove { handle, key } => map(state, handle)?.remove_json(&key),
        Operation::LogLifecycle {
            lifecycle,
            name,
            schema,
        } => {
            let handle = state.handle();
            let log: StoredLog = match &state.client {
                Client::Memory(client) => Arc::new(open_log(client, lifecycle, name, schema)?),
                Client::S3(client) => Arc::new(open_log(client, lifecycle, name, schema)?),
            };
            lock(&state.logs).insert(handle, log);
            json_string(&handle)
        }
        Operation::LogAppend { handle, value_json } => {
            log(state, handle)?.append_json(parse_value(&value_json)?)
        }
        Operation::LogGet { handle, id } => log(state, handle)?.get_json(parse_log_id(&id)?),
        Operation::LogRecords { handle } => log(state, handle)?.records_json(),
        Operation::LogRecordsAfter { handle, id } => {
            log(state, handle)?.records_after_json(parse_log_id(&id)?)
        }
    }
}

fn open_map<S>(
    client: &Objsds<S>,
    lifecycle: Lifecycle,
    name: String,
    schema: String,
) -> NapiResult<Map<S, Value>>
where
    S: ObjectStore,
    S::Error: Display,
{
    let builder = client.map(name).schema(schema);
    match lifecycle {
        Lifecycle::Create => builder.create(),
        Lifecycle::Open => builder.open(),
        Lifecycle::OpenOrCreate => builder.open_or_create(),
    }
    .map_err(operation_error)
}

fn open_log<S>(
    client: &Objsds<S>,
    lifecycle: Lifecycle,
    name: String,
    schema: String,
) -> NapiResult<Log<S, Value>>
where
    S: ObjectStore,
    S::Error: Display,
{
    let builder = client.log(name).schema(schema);
    match lifecycle {
        Lifecycle::Create => builder.create(),
        Lifecycle::Open => builder.open(),
        Lifecycle::OpenOrCreate => builder.open_or_create(),
    }
    .map_err(operation_error)
}

fn map(state: &State, handle: u32) -> NapiResult<StoredMap> {
    lock(&state.maps)
        .get(&handle)
        .map(Arc::clone)
        .ok_or_else(invalid_handle)
}

fn log(state: &State, handle: u32) -> NapiResult<StoredLog> {
    lock(&state.logs)
        .get(&handle)
        .map(Arc::clone)
        .ok_or_else(invalid_handle)
}

fn parse_value(value: &str) -> NapiResult<Value> {
    serde_json::from_str(value).map_err(|error| {
        binding_error(
            "ERR_OBJSDS_INVALID_JSON",
            &format!("invalid JSON value: {error}"),
            json!({}),
        )
    })
}

fn parse_log_id(id: &str) -> NapiResult<objsds::LogId> {
    serde_json::from_value(Value::String(id.to_owned())).map_err(|error| {
        binding_error(
            "ERR_OBJSDS_INVALID_LOG_ID",
            &format!("invalid log identifier: {error}"),
            json!({ "id": id }),
        )
    })
}

fn optional_json<T: serde::Serialize>(value: Option<T>) -> NapiResult<String> {
    match value {
        Some(value) => json_string(&json!({ "found": true, "value": value })),
        None => json_string(&json!({ "found": false })),
    }
}

fn json_string<T: serde::Serialize + ?Sized>(value: &T) -> NapiResult<String> {
    serde_json::to_string(value).map_err(|error| {
        binding_error(
            "ERR_OBJSDS_DOCUMENT",
            &format!("could not encode binding result: {error}"),
            json!({}),
        )
    })
}

fn operation_error<E: Display>(error: Error<E>) -> NapiError {
    match error {
        Error::Configuration(error) => configuration_error(error),
        Error::Store(error) => binding_error("ERR_OBJSDS_STORE", &error.to_string(), json!({})),
        Error::Document(error) => {
            binding_error("ERR_OBJSDS_DOCUMENT", &error.to_string(), json!({}))
        }
        Error::NotFound => binding_error(
            "ERR_OBJSDS_NOT_FOUND",
            "data structure does not exist",
            json!({}),
        ),
        Error::AlreadyExists(error) => binding_error(
            "ERR_OBJSDS_ALREADY_EXISTS",
            "data structure already exists",
            json!({ "observedVersion": error.observed.as_str() }),
        ),
        Error::Conflict(error) => binding_error(
            "ERR_OBJSDS_CONFLICT",
            "object version conflict",
            json!({
                "expectedVersion": error.expected.as_str(),
                "observedVersion": error.observed.as_ref().map(|version| version.as_str()),
            }),
        ),
        Error::Incompatible(error) => {
            binding_error("ERR_OBJSDS_INCOMPATIBLE", &error.to_string(), json!({}))
        }
    }
}

fn configuration_error(error: impl Display) -> NapiError {
    binding_error(
        "ERR_OBJSDS_INVALID_CONFIGURATION",
        &error.to_string(),
        json!({}),
    )
}

fn invalid_handle() -> NapiError {
    binding_error(
        "ERR_OBJSDS_INVALID_HANDLE",
        "native data-structure handle is invalid",
        json!({}),
    )
}

fn binding_error(code: &str, message: &str, details: Value) -> NapiError {
    let envelope = json!({ "code": code, "message": message, "details": details });
    NapiError::new(
        Status::GenericFailure,
        format!("OBJSDS_ERR_JSON:{envelope}"),
    )
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
