//! Read-only sudo authorization and browsing on the shared session cache.
use super::*;

#[derive(Deserialize)]
struct Authorization {
    session_id: String,
    password: Option<String>,
}
impl Drop for Authorization {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn list(
    request_id: u64,
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
    private_key: Option<&str>,
    certificate: Option<&str>,
    passphrase: Option<&str>,
    known_hosts: Option<&str>,
    proxy: Option<SshProxyConfig>,
    directory: &str,
    trust: HostKeyTrustMode,
    session_id: &str,
    sudo_password: Option<&str>,
) -> SftpDirectoryEntries {
    let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
    let cancel = sftp_task_cancel_flag(request_id);
    let result = (|| {
        if session_id.is_empty() || session_id.len() > 256 {
            return Err("Invalid sudo session identifier".to_owned());
        }
        if !directory.starts_with('/') || directory.len() > 64 * 1024 {
            return Err("Choose an absolute SFTP directory for sudo authorization".to_owned());
        }
        let authorization = Authorization {
            session_id: session_id.to_owned(),
            password: sudo_password.map(str::to_owned),
        };
        let password = password.unwrap_or_default();
        let private_key = private_key.unwrap_or_default();
        let certificate = certificate.unwrap_or_default();
        let passphrase = passphrase.unwrap_or_default();
        let known_hosts = known_hosts.unwrap_or_default();
        fn optional(value: &str) -> Option<&str> {
            (!value.is_empty()).then_some(value)
        }
        sudo_sftp_runtime()?.block_on(async {
            let session = {
                let cached = sudo_sftp_sessions()
                    .lock()
                    .await
                    .get(&authorization.session_id)
                    .cloned();
                if let Some(session) = cached.as_ref() {
                    let session = session.lock().await;
                    if session.authenticated_at.elapsed() >= SUDO_SFTP_SESSION_LIFETIME {
                        return Err("sudo SFTP session expired after 15 minutes".to_owned());
                    }
                }
                match cached {
                    Some(session) => session,
                    None => {
                        let sudo_password = authorization
                            .password
                            .as_deref()
                            .ok_or_else(|| "sudo SFTP session is not authenticated".to_owned())?;
                        let (handle, sftp) = cancellable(
                            &cancel,
                            open_sudo_sftp_session(
                                host,
                                port,
                                username,
                                optional(password),
                                optional(private_key),
                                optional(certificate),
                                optional(passphrase),
                                optional(known_hosts),
                                proxy.as_ref(),
                                trust,
                                events.clone(),
                                sudo_password,
                                8,
                            ),
                        )
                        .await?;
                        let session = Arc::new(tokio::sync::Mutex::new(CachedSudoSftpSession {
                            handle,
                            sftp,
                            authenticated_at: Instant::now(),
                        }));
                        sudo_sftp_sessions()
                            .lock()
                            .await
                            .insert(authorization.session_id.clone(), session.clone());
                        session
                    }
                }
            };
            cancellable(&cancel, async {
                let session = session.lock().await;
                let directory = session
                    .sftp
                    .canonicalize(directory)
                    .await
                    .map_err(|error| format!("Could not resolve sudo directory: {error}"))?;
                let read_dir = session
                    .sftp
                    .read_dir(directory.clone())
                    .await
                    .map_err(|error| format!("Could not read sudo directory: {error}"))?;
                let mut entries = vec![];
                for entry in read_dir {
                    if cancel.load(Ordering::SeqCst) {
                        return Err("SFTP listing cancelled".into());
                    }
                    let name = entry.file_name();
                    if name.is_empty() || name == "." || name == ".." {
                        continue;
                    }
                    if name.contains(['/', '\0']) {
                        return Err("Invalid remote filename".into());
                    }
                    let path = format!("{}/{name}", directory.trim_end_matches('/'));
                    let metadata = session.sftp.metadata(path).await.ok();
                    entries.push(SshDirectoryEntry {
                        name,
                        is_directory: metadata
                            .as_ref()
                            .map(|m| m.is_dir())
                            .unwrap_or_else(|| entry.file_type().is_dir()),
                        size: metadata.as_ref().and_then(|m| m.size).unwrap_or(0),
                        modified: metadata.as_ref().and_then(|m| m.mtime).map(u64::from),
                        mode: metadata.as_ref().and_then(|m| m.permissions),
                    });
                }
                Ok((directory, entries))
            })
            .await
        })
    })();
    finish_sftp_task(request_id);
    let events = events
        .lock()
        .map(|events| events.clone())
        .unwrap_or_default();
    match result {
        Ok((directory, entries)) => SftpDirectoryEntries {
            directory,
            entries,
            events,
            error: None,
        },
        Err(error) => SftpDirectoryEntries {
            directory: String::new(),
            entries: vec![],
            events,
            error: Some(error),
        },
    }
}

async fn cancellable<T>(
    cancel: &AtomicBool,
    work: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::pin!(work);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("SFTP listing cancelled".into());
        }
        tokio::select! {
            result = &mut work => return result,
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
    }
}
