//! In-process SSH/SFTP server backed by a private temporary directory.
//! Exercises the production upload/download/listing code over actual sockets.
use super::*;
use russh::server;
use russh_sftp::protocol::{
    Attrs, Data, File as RemoteFile, FileAttributes, Handle, Name, Status, StatusCode,
};
use std::collections::{HashMap, HashSet};

struct Fixture {
    address: std::net::SocketAddr,
    root: tempfile::TempDir,
    trust: tempfile::TempDir,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let key: russh::keys::ssh_key::PrivateKey =
            russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[61; 32]).into();
        let trust = tempfile::tempdir().unwrap();
        russh::keys::known_hosts::learn_known_hosts_path(
            "127.0.0.1",
            address.port(),
            key.public_key(),
            trust.path().join("known_hosts"),
        )
        .unwrap();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::ZERO,
            ..Default::default()
        });
        let directory = root.path().to_owned();
        let server = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let session = server::run_stream(
                    config.clone(),
                    stream,
                    SshHandler {
                        root: directory.clone(),
                        channels: HashMap::new(),
                    },
                )
                .await
                .unwrap();
                tokio::spawn(async move {
                    let _ = session.await;
                });
            }
        });
        Self {
            address,
            root,
            trust,
            server,
        }
    }
    async fn session(&self) -> (client::Handle<SshClientHandler>, SftpSession) {
        open_sftp_session(
            "127.0.0.1",
            self.address.port(),
            "fixture",
            Some("fixture-only-password"),
            None,
            None,
            None,
            Some(self.trust.path().join("known_hosts").to_str().unwrap()),
            None,
            HostKeyTrustMode::Strict,
            Arc::new(Mutex::new(vec![])),
            4,
        )
        .await
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
struct SshHandler {
    root: PathBuf,
    channels: HashMap<russh::ChannelId, russh::Channel<server::Msg>>,
}
impl server::Handler for SshHandler {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(
            if user == "fixture" && password == "fixture-only-password" {
                server::Auth::Accept
            } else {
                server::Auth::Reject {
                    proceed_with_methods: None,
                    partial_success: false,
                }
            },
        )
    }
    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        channel: russh::ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(name, "sftp");
        let channel_stream = self.channels.remove(&channel).unwrap().into_stream();
        session.channel_success(channel)?;
        russh_sftp::server::run(
            channel_stream,
            Files {
                root: self.root.clone(),
                files: HashMap::new(),
                directories: HashMap::new(),
                next: 0,
            },
        )
        .await;
        Ok(())
    }
}

struct Files {
    root: PathBuf,
    files: HashMap<String, std::fs::File>,
    directories: HashMap<String, Option<Vec<RemoteFile>>>,
    next: u64,
}
fn status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: "en".into(),
    }
}
fn error(error: std::io::Error) -> StatusCode {
    match error.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
        std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}
fn attributes(metadata: &std::fs::Metadata) -> FileAttributes {
    FileAttributes {
        size: Some(metadata.len()),
        permissions: Some(if metadata.is_dir() {
            0o040755
        } else {
            0o100644
        }),
        mtime: Some(1),
        atime: Some(1),
        ..Default::default()
    }
}
impl Files {
    fn path(&self, path: &str) -> Result<PathBuf, StatusCode> {
        let mut result = self.root.clone();
        for part in path.split('/') {
            match part {
                "" | "." | "~" => {}
                ".." => return Err(StatusCode::PermissionDenied),
                part if part.contains(['\\', ':', '\0']) => {
                    return Err(StatusCode::PermissionDenied)
                }
                part => result.push(part),
            }
        }
        Ok(result)
    }
    fn handle(&mut self) -> String {
        self.next += 1;
        self.next.to_string()
    }
}
impl russh_sftp::server::Handler for Files {
    type Error = StatusCode;
    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let options: std::fs::OpenOptions = flags.into();
        let file = options.open(self.path(&filename)?).map_err(error)?;
        let handle = self.handle();
        self.files.insert(handle.clone(), file);
        Ok(Handle { id, handle })
    }
    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.files.remove(&handle);
        self.directories.remove(&handle);
        Ok(status(id))
    }
    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: attributes(&std::fs::metadata(self.path(&path)?).map_err(error)?),
        })
    }
    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.stat(id, path).await
    }
    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: attributes(
                &self
                    .files
                    .get(&handle)
                    .ok_or(StatusCode::Failure)?
                    .metadata()
                    .map_err(error)?,
            ),
        })
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let file = self.files.get_mut(&handle).ok_or(StatusCode::Failure)?;
        std::io::Seek::seek(file, std::io::SeekFrom::Start(offset)).map_err(error)?;
        let mut data = vec![0; len.min(256 * 1024) as usize];
        let read = std::io::Read::read(file, &mut data).map_err(error)?;
        if read == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(read);
        Ok(Data { id, data })
    }
    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let file = self.files.get_mut(&handle).ok_or(StatusCode::Failure)?;
        std::io::Seek::seek(file, std::io::SeekFrom::Start(offset)).map_err(error)?;
        std::io::Write::write_all(file, &data).map_err(error)?;
        Ok(status(id))
    }
    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let entries = std::fs::read_dir(self.path(&path)?)
            .map_err(error)?
            .map(|entry| {
                let entry = entry.map_err(error)?;
                Ok(RemoteFile::new(
                    entry.file_name().to_str().unwrap(),
                    attributes(&entry.metadata().map_err(error)?),
                ))
            })
            .collect::<Result<Vec<_>, StatusCode>>()?;
        let handle = self.handle();
        self.directories.insert(handle.clone(), Some(entries));
        Ok(Handle { id, handle })
    }
    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        let files = self
            .directories
            .get_mut(&handle)
            .ok_or(StatusCode::Failure)?
            .take()
            .ok_or(StatusCode::Eof)?;
        Ok(Name { id, files })
    }
    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let resolved = self.path(&path)?;
        let relative = resolved
            .strip_prefix(&self.root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        Ok(Name {
            id,
            files: vec![RemoteFile::dummy(format!("/{relative}"))],
        })
    }
    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _: FileAttributes,
    ) -> Result<Status, Self::Error> {
        std::fs::create_dir(self.path(&path)?).map_err(error)?;
        Ok(status(id))
    }
    async fn rename(&mut self, id: u32, from: String, to: String) -> Result<Status, Self::Error> {
        std::fs::rename(self.path(&from)?, self.path(&to)?).map_err(error)?;
        Ok(status(id))
    }
    async fn remove(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        std::fs::remove_file(self.path(&path)?).map_err(error)?;
        Ok(status(id))
    }
    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        std::fs::remove_dir(self.path(&path)?).map_err(error)?;
        Ok(status(id))
    }
}

fn progress() -> SftpTaskProgress {
    SftpTaskProgress {
        callback: None,
        cancel: Arc::new(AtomicBool::new(false)),
        total_bytes: 0,
        transferred_bytes: 0,
        concurrent: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sftp_round_trips_windows_local_paths_and_unicode_remote_directory_tree() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let fixture = Fixture::new().await;
        let local = tempfile::tempdir().unwrap();
        let source = local.path().join("本地 & source");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(source.join("子目录")).unwrap();
        let payload = vec![0x5a; 2 * 1024 * 1024 + 57];
        std::fs::write(source.join("报告 & O'Brien.txt"), &payload).unwrap();
        std::fs::write(source.join("子目录").join("nested.txt"), b"nested").unwrap();
        let (handle, sftp) = fixture.session().await;
        let mut upload = progress();
        let result = upload_sftp_path(&sftp, &source, "/远程 & folder", false, 4, &mut upload)
            .await
            .unwrap();
        assert_eq!(result, (payload.len() as u64 + 6, "folder".into()));
        assert_eq!(
            std::fs::read(fixture.root.path().join("远程 & folder/报告 & O'Brien.txt")).unwrap(),
            payload
        );

        let listing = list_sftp_directory_entries(
            "127.0.0.1",
            fixture.address.port(),
            "fixture",
            Some("fixture-only-password"),
            None,
            None,
            None,
            Some(fixture.trust.path().join("known_hosts").to_str().unwrap()),
            None,
            "/远程 & folder",
            HostKeyTrustMode::Strict,
            Arc::new(AtomicBool::new(false)),
        )
        .await;
        assert!(listing.error.is_none(), "{:?}", listing.error);
        assert_eq!(listing.directory, "/远程 & folder");
        assert_eq!(
            listing
                .entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<HashSet<_>>(),
            HashSet::from(["子目录", "报告 & O'Brien.txt"])
        );
        let target = local.path().join("下载 & target");
        let mut download = progress();
        let result = download_sftp_path(&sftp, "/远程 & folder", &target, 4, &mut download)
            .await
            .unwrap();
        assert_eq!(result.0, upload.total_bytes);
        assert_eq!(
            std::fs::read(target.join("报告 & O'Brien.txt")).unwrap(),
            payload
        );
        assert_eq!(
            std::fs::read(target.join("子目录/nested.txt")).unwrap(),
            b"nested"
        );
        assert!(!local_download_staging_path(&target).exists());
        assert!(!local_part_metadata_path(&local_download_staging_path(&target)).exists());
        sftp.close().await.unwrap();
        handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await
            .unwrap();
    })
    .await
    .expect("SFTP fixture timed out");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_sftp_upload_preserves_destination_and_resumes_owned_staging() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let fixture = Fixture::new().await;
        let local = tempfile::tempdir().unwrap();
        let source = local.path().join("中文 & cancelled.txt");
        let payload = vec![0x6b; 2 * 1024 * 1024 + 13];
        std::fs::write(&source, &payload).unwrap();
        std::fs::write(fixture.root.path().join("target.txt"), b"original").unwrap();
        let (handle, sftp) = fixture.session().await;
        let mut cancelled = progress();
        let flag = cancelled.cancel.clone();
        cancelled.callback = Some(ProgressDelivery::Rust(Arc::new(move |bytes, _, _| {
            if bytes >= SFTP_TRANSFER_BUFFER_SIZE as u64 {
                flag.store(true, Ordering::SeqCst);
            }
        })));
        let error = upload_sftp_path(&sftp, &source, "/target.txt", true, 4, &mut cancelled)
            .await
            .unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
        assert_eq!(
            std::fs::read(fixture.root.path().join("target.txt")).unwrap(),
            b"original"
        );
        assert!(sftp
            .metadata(remote_upload_staging_path("/target.txt"))
            .await
            .is_ok());
        let mut resumed = progress();
        upload_sftp_path(&sftp, &source, "/target.txt", true, 4, &mut resumed)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(fixture.root.path().join("target.txt")).unwrap(),
            payload
        );
        assert!(sftp
            .metadata(remote_upload_staging_path("/target.txt"))
            .await
            .is_err());
        sftp.close().await.unwrap();
        handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await
            .unwrap();
    })
    .await
    .expect("SFTP cancellation fixture timed out");
}
