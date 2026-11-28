use super::*;

pub(super) async fn authorize(
    config: config::Config,
    client: reqwest::Client,
    events: &async_channel::Sender<Event>,
    language: crate::localization::Language,
) -> Result<Authorized> {
    let server = callback::Server::bind(&config).await?;
    let verifier = config::verifier();
    let state = config::state();
    let url = config.authorization_uri(&server.redirect, &state, &config::challenge(&verifier));
    progress(events, Event::Browser(url.into())).await?;
    let code = server.code(&state, language).await?;
    progress(events, Event::Exchanging).await?;
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("client_id", config.client_id.as_str()),
        ("code", code.as_str()),
        ("redirect_uri", server.redirect.as_str()),
        ("code_verifier", verifier.as_str()),
    ];
    if !config.client_secret.is_empty() {
        form.push(("client_secret", config.client_secret.as_str()));
    }
    let response = client
        .post(config.token_uri())
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
        .map_err(network_error)?;
    let (status, mut tokens): (_, Tokens) = payload(response).await?;
    let credentials = config.credentials(&mut tokens, status)?;
    Ok(Authorized {
        credentials,
        account: None,
    })
}
