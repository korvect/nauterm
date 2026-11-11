use super::*;
use russh::{server, Channel, ChannelId};
use russh_sftp::protocol::{Attrs, File, FileAttributes, Handle, Name, Status, StatusCode};
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::net::SocketAddr;
use tokio::task::JoinSet;

struct LegacyServer {
    address: SocketAddr,
    known_hosts: PathBuf,
    key: russh::keys::ssh_key::PrivateKey,
    worker: tokio::task::JoinHandle<()>,
}

impl LegacyServer {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let key: russh::keys::ssh_key::PrivateKey =
            russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[42; 32]).into();
        let known_hosts = std::env::temp_dir().join(format!(
            "nauterm-compatibility-{}-{}",
            std::process::id(),
            address.port()
        ));
        learn_known_hosts_path_without_leading_blank(
            "127.0.0.1",
            address.port(),
            key.public_key(),
            &known_hosts,
        )
        .unwrap();
        let config = Arc::new(server::Config {
            keys: vec![key.clone()],
            preferred: russh::Preferred {
                kex: Cow::Borrowed(&[russh::kex::DH_G14_SHA1]),
                ..russh::Preferred::DEFAULT
            },
            auth_rejection_time: Duration::ZERO,
            ..Default::default()
        });
        let worker = tokio::spawn(async move {
            let mut sessions = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.unwrap();
                        let config = config.clone();
                        sessions.spawn(async move {
                            if let Ok(session) = server::run_stream(config, stream, LegacySession::default()).await {
                                let _ = session.await;
                            }
                        });
                    }
                    _ = sessions.join_next(), if !sessions.is_empty() => {}
                }
            }
        });
        Self {
            address,
            known_hosts,
            key,
            worker,
        }
    }

    fn path(&self) -> &str {
        self.known_hosts.to_str().unwrap()
    }
}

impl Drop for LegacyServer {
    fn drop(&mut self) {
        self.worker.abort();
        let _ = fs::remove_file(&self.known_hosts);
    }
}

#[derive(Default)]
struct LegacySession {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    subsystems: JoinSet<()>,
}

impl server::Handler for LegacySession {
    type Error = russh::Error;

    async fn auth_none(&mut self, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(name, "sftp");
        session.channel_success(id)?;
        let channel = self.channels.remove(&id).unwrap();
        self.subsystems.spawn(async move {
            russh_sftp::server::run(channel.into_stream(), TestSftp::default()).await;
        });
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: ChannelId,
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(command);
        session.channel_success(id)?;
        if command.starts_with("sudo ") {
            let mut stream = self.channels.remove(&id).unwrap().into_stream();
            self.subsystems.spawn(async move {
                while stream.read_u8().await.unwrap() != b'\n' {}
                russh_sftp::server::run(stream, TestSftp::default()).await;
            });
            return Ok(());
        }
        let output: &[u8] = if command.contains("mosh-server") {
            b"MOSH CONNECT 60001 AAAAAAAAAAAAAAAAAAAAAA\n"
        } else if command.starts_with("dir=") {
            b"p\t/\0f\tconsole.txt\0"
        } else if command.contains("nauterm_emit") {
            b"hostname=appliance\nkernel=TestOS\n"
        } else {
            b""
        };
        session.data(id, output.to_vec())?;
        session.exit_status_request(id, 0)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
}

#[derive(Default)]
struct TestSftp {
    listed: bool,
}

impl russh_sftp::server::Handler for TestSftp {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        StatusCode::OpUnsupported
    }

    async fn realpath(&mut self, id: u32, _: String) -> Result<Name, StatusCode> {
        Ok(Name {
            id,
            files: vec![File::dummy("/")],
        })
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, StatusCode> {
        self.listed = false;
        Ok(Handle { id, handle: path })
    }

    async fn readdir(&mut self, id: u32, _: String) -> Result<Name, StatusCode> {
        if self.listed {
            return Err(StatusCode::Eof);
        }
        self.listed = true;
        Ok(Name {
            id,
            files: vec![File::new("console.txt", file_attributes())],
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        if path != "/" && path != "/console.txt" {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Attrs {
            id,
            attrs: file_attributes(),
        })
    }

    async fn close(&mut self, id: u32, _: String) -> Result<Status, StatusCode> {
        Ok(ok_status(id))
    }

    async fn mkdir(&mut self, id: u32, _: String, _: FileAttributes) -> Result<Status, StatusCode> {
        Ok(ok_status(id))
    }
}

fn file_attributes() -> FileAttributes {
    FileAttributes {
        size: Some(7),
        permissions: Some(0o100644),
        ..Default::default()
    }
}

fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}

fn run_test(future: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(10), future)
                .await
                .unwrap()
        });
}

fn decode_ffi_json(pointer: *mut c_char) -> serde_json::Value {
    assert!(!pointer.is_null());
    let text = unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe {
        crate::nauterm_string_free(pointer);
    }
    serde_json::from_str(&text).unwrap()
}

#[test]
fn compatibility_sftp_ffi_listing_and_regular_and_sudo_tasks() {
    run_test(async {
        let server = LegacyServer::start().await;
        for enabled in [false, true] {
            let port = server.address.port();
            let known_hosts = server.path().to_owned();
            let listing = tokio::task::spawn_blocking(move || {
                let host = CString::new("127.0.0.1").unwrap();
                let user = CString::new("admin").unwrap();
                let path = CString::new(known_hosts).unwrap();
                let directory = CString::new("/").unwrap();
                decode_ffi_json(crate::nauterm_sftp_list_directory_entries(
                    u64::from(port) << 32,
                    host.as_ptr(),
                    port,
                    user.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    path.as_ptr(),
                    directory.as_ptr(),
                    0,
                    ptr::null(),
                    enabled,
                ))
            })
            .await
            .unwrap();
            if enabled {
                assert!(listing["error"].is_null(), "{listing}");
                assert_eq!(listing["entries"][0]["name"], "console.txt");
            } else {
                assert!(listing["error"].as_str().unwrap().contains("No common Kex"));
            }

            for sudo in [false, true] {
                let known_hosts = server.path().to_owned();
                let result = tokio::task::spawn_blocking(move || {
                    let host = CString::new("127.0.0.1").unwrap();
                    let user = CString::new("admin").unwrap();
                    let path = CString::new(known_hosts).unwrap();
                    let session_id = format!("compatibility-sudo-{port}");
                    let operation = CString::new(
                        serde_json::json!({
                            "op": "mkdir", "target_path": "/new-dir",
                            "sudo_password": sudo.then_some("test-only-password"),
                            "sudo_session_id": sudo.then_some(&session_id),
                        })
                        .to_string(),
                    )
                    .unwrap();
                    let result = decode_ffi_json(crate::nauterm_sftp_execute_task(
                        (u64::from(port) << 32) + 1 + u64::from(sudo),
                        host.as_ptr(),
                        port,
                        user.as_ptr(),
                        ptr::null(),
                        ptr::null(),
                        ptr::null(),
                        ptr::null(),
                        path.as_ptr(),
                        operation.as_ptr(),
                        0,
                        ptr::null(),
                        None,
                        ptr::null_mut(),
                        enabled,
                    ));
                    if sudo && enabled {
                        let session_id = CString::new(session_id).unwrap();
                        assert!(crate::nauterm_sftp_close_sudo_session(session_id.as_ptr()));
                    }
                    result
                })
                .await
                .unwrap();
                assert_eq!(result["ok"], enabled, "{result}");
                if !enabled {
                    assert!(result["error"].as_str().unwrap().contains("No common Kex"));
                }
            }
        }
    });
}

#[test]
fn compatibility_mosh_bootstrap_uses_terminal_options() {
    run_test(async {
        let server = LegacyServer::start().await;
        for enabled in [false, true] {
            let mut options = TerminalOptions::default();
            if enabled {
                options
                    .environment
                    .push(crate::terminal::TerminalEnvironmentVariable {
                        variable: SSH_COMPATIBILITY_MODE_ENV.to_owned(),
                        value: "1".to_owned(),
                    });
            }
            let result = start_mosh_server(
                "127.0.0.1",
                server.address.port(),
                "admin",
                None,
                None,
                None,
                None,
                Some(server.path()),
                None,
                HostKeyTrustMode::Strict,
                &MoshServerCommand::default(),
                &options,
            )
            .await;
            if enabled {
                let result = result.unwrap_or_else(|(error, _)| panic!("{error}"));
                assert_eq!(result.bootstrap.port, 60001);
                assert_eq!(result.bootstrap.key, "AAAAAAAAAAAAAAAAAAAAAA");
            } else {
                assert!(result.err().unwrap().0.contains("No common Kex"));
            }
        }
    });
}

#[test]
fn compatibility_directory_completion_system_info_and_key_export() {
    run_test(async {
        let server = LegacyServer::start().await;
        let public_key = server.key.public_key().to_openssh().unwrap();
        for enabled in [false, true] {
            let listing = list_directory_entries(
                "127.0.0.1",
                server.address.port(),
                "admin",
                None,
                None,
                None,
                None,
                Some(server.path()),
                "/",
                enabled,
            )
            .await;
            if enabled {
                assert_eq!(listing.unwrap().entries[0].name, "console.txt");
            } else {
                assert!(listing.unwrap_err().contains("No common Kex"));
            }
            let info = collect_host_system_info(
                "127.0.0.1",
                server.address.port(),
                "admin",
                None,
                None,
                None,
                None,
                Some(server.path()),
                None,
                HostKeyTrustMode::Strict,
                enabled,
            )
            .await;
            if enabled {
                assert!(info.error.is_none(), "{:?}", info.error);
                assert_eq!(info.hostname.as_deref(), Some("appliance"));
            } else {
                assert!(info.error.unwrap().contains("No common Kex"));
            }
            let exported = export_public_key(
                "127.0.0.1",
                server.address.port(),
                "admin",
                None,
                None,
                None,
                None,
                Some(server.path()),
                None,
                &public_key,
                ".ssh",
                "authorized_keys",
                "exit 0",
                HostKeyTrustMode::Strict,
                enabled,
            )
            .await;
            assert_eq!(exported.ok, enabled, "{:?}", exported.error);
            if !enabled {
                assert!(exported.error.unwrap().contains("No common Kex"));
            }
        }
    });
}

#[test]
fn compatibility_port_forward_uses_saved_connection_policy() {
    run_test(async {
        let server = LegacyServer::start().await;
        let mut manager = crate::port_forward::PortForwardManager::default();
        for enabled in [false, true] {
            manager.start(crate::port_forward::PortForwardConfig {
                id: 1,
                forward_type: "local".to_owned(),
                ssh_host: "127.0.0.1".to_owned(),
                ssh_port: server.address.port(),
                username: "admin".to_owned(),
                password: None,
                private_key: None,
                certificate: None,
                passphrase: None,
                known_hosts_path: Some(server.path().to_owned()),
                proxy: None,
                ssh_compatibility_mode: enabled,
                bind_address: "127.0.0.1".to_owned(),
                bind_port: 0,
                destination_host: Some("127.0.0.1".to_owned()),
                destination_port: 9,
            });
            let status = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let status = manager.status(1);
                    if status.state != "starting" {
                        break status;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            if enabled {
                assert_eq!(status.state, "running", "{:?}", status.error);
                assert!(status.bound_port.is_some_and(|port| port > 0));
            } else {
                assert_eq!(status.state, "error");
                assert!(status.error.unwrap().contains("No common Kex"));
            }
            manager.stop(1);
        }
    });
}
