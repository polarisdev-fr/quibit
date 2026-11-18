use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const CONNECTION_RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QemuConfig {
    pub machine: String,
    pub accelerator: String,
    pub cpu: String,
    pub memory_mb: u16,
}

impl Default for QemuConfig {
    fn default() -> Self {
        Self {
            machine: "virt".to_owned(),
            accelerator: "hvf".to_owned(),
            cpu: "host".to_owned(),
            memory_mb: 512,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VmStatus {
    pub status: String,
    pub running: bool,
}

#[derive(Debug)]
pub enum QmpError {
    ProcessSpawn(io::Error),
    ConnectionTimeout(PathBuf),
    Io(io::Error),
    InvalidJson(serde_json::Error),
    UnexpectedReply(String),
    VmAlreadyRunning(String),
    VmNotFound(String),
    VmExited(String),
    SocketPathTooLong(PathBuf),
}

impl Serialize for QmpError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl fmt::Display for QmpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessSpawn(error) => write!(formatter, "failed to spawn QEMU: {error}"),
            Self::ConnectionTimeout(socket_path) => write!(
                formatter,
                "timed out connecting to QMP socket {} after {:.1} seconds",
                socket_path.display(),
                CONNECTION_TIMEOUT.as_secs_f32()
            ),
            Self::Io(error) => write!(formatter, "QMP I/O error: {error}"),
            Self::InvalidJson(error) => write!(formatter, "invalid QMP JSON: {error}"),
            Self::UnexpectedReply(reply) => write!(formatter, "unexpected QMP reply: {reply}"),
            Self::VmAlreadyRunning(vm_id) => {
                write!(formatter, "VM '{vm_id}' is already managed")
            }
            Self::VmNotFound(vm_id) => write!(formatter, "VM '{vm_id}' was not found"),
            Self::VmExited(vm_id) => write!(formatter, "QEMU process for VM '{vm_id}' has exited"),
            Self::SocketPathTooLong(path) => write!(
                formatter,
                "QMP socket path {} exceeds the Unix socket path limit",
                path.display()
            ),
        }
    }
}

impl Error for QmpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProcessSpawn(error) | Self::Io(error) => Some(error),
            Self::InvalidJson(error) => Some(error),
            Self::ConnectionTimeout(_)
            | Self::UnexpectedReply(_)
            | Self::VmAlreadyRunning(_)
            | Self::VmNotFound(_)
            | Self::VmExited(_)
            | Self::SocketPathTooLong(_) => None,
        }
    }
}

impl From<io::Error> for QmpError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for QmpError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidJson(error)
    }
}

pub async fn spawn_and_connect(
    socket_path: &Path,
    config: &QemuConfig,
) -> Result<(Child, BufReader<UnixStream>), QmpError> {
    if socket_path.as_os_str().as_encoded_bytes().len() >= 104 {
        return Err(QmpError::SocketPathTooLong(socket_path.to_owned()));
    }

    let machine = format!("{},accel={}", config.machine, config.accelerator);
    let qmp_socket = format!("unix:{},server,nowait", socket_path.display());
    let child = Command::new("qemu-system-aarch64")
        .args([
            "-machine",
            &machine,
            "-cpu",
            &config.cpu,
            "-m",
            &config.memory_mb.to_string(),
            "-display",
            "none",
            "-nodefaults",
            "-S",
            "-qmp",
            &qmp_socket,
        ])
        .kill_on_drop(true)
        .spawn()
        .map_err(QmpError::ProcessSpawn)?;

    let stream = connect_to_qmp(socket_path).await?;
    let mut reader = BufReader::new(stream);

    read_greeting(&mut reader).await?;
    let capabilities_reply = execute(&mut reader, "qmp_capabilities").await?;
    if !capabilities_reply
        .as_object()
        .is_some_and(|object| object.is_empty())
    {
        return Err(QmpError::UnexpectedReply(
            "qmp_capabilities did not return an empty object".to_owned(),
        ));
    }

    Ok((child, reader))
}

pub async fn query_status(reader: &mut BufReader<UnixStream>) -> Result<VmStatus, QmpError> {
    let status_reply = execute(reader, "query-status").await?;
    Ok(serde_json::from_value(status_reply)?)
}

async fn connect_to_qmp(socket_path: &Path) -> Result<UnixStream, QmpError> {
    let connection = timeout(CONNECTION_TIMEOUT, async {
        loop {
            if let Ok(stream) = UnixStream::connect(socket_path).await {
                return stream;
            }
            sleep(CONNECTION_RETRY_DELAY).await;
        }
    })
    .await;

    connection.map_err(|_| QmpError::ConnectionTimeout(socket_path.to_owned()))
}

async fn read_greeting(reader: &mut BufReader<UnixStream>) -> Result<(), QmpError> {
    let greeting = read_json_line(reader).await?;
    if greeting.get("QMP").is_none() {
        return Err(QmpError::UnexpectedReply(
            "QMP greeting is missing the QMP key".to_owned(),
        ));
    }
    Ok(())
}

async fn execute(reader: &mut BufReader<UnixStream>, command: &str) -> Result<Value, QmpError> {
    let mut request = serde_json::to_vec(&json!({ "execute": command }))?;
    request.push(b'\n');
    reader.get_mut().write_all(&request).await?;
    reader.get_mut().flush().await?;
    wait_for_reply(reader).await
}

async fn wait_for_reply(reader: &mut BufReader<UnixStream>) -> Result<Value, QmpError> {
    loop {
        let message = read_json_line(reader).await?;
        if message.get("event").is_some() {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(QmpError::UnexpectedReply(format!(
                "QMP command failed: {error}"
            )));
        }
        if let Some(reply) = message.get("return") {
            return Ok(reply.clone());
        }
        return Err(QmpError::UnexpectedReply(format!(
            "QMP response has no event, return, or error key: {message}"
        )));
    }
}

async fn read_json_line(reader: &mut BufReader<UnixStream>) -> Result<Value, QmpError> {
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        return Err(QmpError::UnexpectedReply(
            "QMP socket closed before a reply was received".to_owned(),
        ));
    }
    Ok(serde_json::from_str(line.trim_end())?)
}
