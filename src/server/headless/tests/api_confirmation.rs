use super::*;

const SHELL_CLIENT: u64 = 21;

fn attach_shell_client(server: &mut HeadlessServer) {
    server.clients.insert(
        SHELL_CLIENT,
        ClientConnection::new_with_mode(
            ClientConnectionMode::ClientShell,
            (80, 24),
            crate::kitty_graphics::HostCellSize::default(),
            1,
            RenderEncoding::SemanticFrame,
            None,
        ),
    );
}

fn socket_request(
    server: &mut HeadlessServer,
    method: api::schema::Method,
) -> std::sync::mpsc::Receiver<String> {
    let (respond_to, response_rx) = std::sync::mpsc::channel();
    server.handle_api_request_with_shutdown_check(api::ApiRequestMessage {
        request: api::schema::Request {
            id: "confirm-me".into(),
            method,
        },
        respond_to,
        response_write_complete: None,
    });
    response_rx
}

fn answer(server: &mut HeadlessServer, client_id: u64, id: u64, allow: bool) -> bool {
    server.handle_server_event(ServerEvent::ClientShellApiConfirmation {
        client_id,
        answer: protocol::endpoint::EndpointApiConfirmationAnswer { id, allow },
    })
}

fn error_of(response: &str) -> (String, String) {
    let response: serde_json::Value = serde_json::from_str(response).unwrap();
    assert_eq!(response["id"], "confirm-me", "{response}");
    (
        response["error"]["code"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        response["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
    )
}

fn integration_install() -> api::schema::Method {
    api::schema::Method::IntegrationInstall(api::schema::IntegrationInstallParams {
        target: api::schema::IntegrationTarget::Claude,
    })
}

fn plugin_enable(plugin_id: &str) -> api::schema::Method {
    api::schema::Method::PluginEnable(api::schema::PluginSetEnabledParams {
        plugin_id: plugin_id.into(),
    })
}

fn temp_plugin_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "h0x-api-confirm-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("h0x-plugin.toml"),
        r#"
id = "example.confirm"
name = "Confirm"
version = "0.1.0"
min_h0x_version = "0.6.10"
platforms = ["linux", "macos", "windows"]
"#,
    )
    .unwrap();
    root
}

fn install_disabled_plugin(server: &mut HeadlessServer) -> PathBuf {
    let root = temp_plugin_root("enable");
    let plugin = crate::app::load_plugin_manifest(&root.display().to_string(), false).unwrap();
    server
        .app
        .state
        .installed_plugins
        .insert(plugin.plugin_id.clone(), plugin);
    root
}

fn pending_id(server: &HeadlessServer) -> u64 {
    server
        .app
        .current_api_confirmation()
        .expect("pending api confirmation")
        .id
}

#[test]
fn api_integration_install_from_socket_is_pending_and_shown_to_clients() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);

    let response_rx = socket_request(&mut server, integration_install());

    assert!(response_rx.try_recv().is_err(), "request must wait");
    let pending = server.app.current_api_confirmation().unwrap();
    assert_eq!(pending.prompt, "Install the claude integration hooks");
    let snapshot = crate::server::client_shell::snapshot(&server.app, "boot", 1, None, None);
    assert_eq!(snapshot.api_confirmation, Some(pending.clone()));

    assert!(answer(&mut server, SHELL_CLIENT, pending.id, false));
    let (code, message) = error_of(&response_rx.recv_timeout(Duration::from_secs(1)).unwrap());
    assert_eq!(code, "denied_by_user");
    assert_eq!(message, "denied by user");
    assert!(server.app.current_api_confirmation().is_none());
    let snapshot = crate::server::client_shell::snapshot(&server.app, "boot", 2, None, None);
    assert_eq!(snapshot.api_confirmation, None);
    shutdown_test_runtimes(&mut server);
}

#[tokio::test]
async fn api_plugin_enable_allowed_by_user_runs_and_returns_the_result() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let root = install_disabled_plugin(&mut server);

    let response_rx = socket_request(&mut server, plugin_enable("example.confirm"));
    assert!(response_rx.try_recv().is_err(), "request must wait");
    assert!(!server.app.state.installed_plugins["example.confirm"].enabled);
    assert_eq!(
        server.app.current_api_confirmation().unwrap().prompt,
        "Enable plugin example.confirm"
    );

    let id = pending_id(&server);
    assert!(answer(&mut server, SHELL_CLIENT, id, true));
    let response: serde_json::Value =
        serde_json::from_str(&response_rx.recv_timeout(Duration::from_secs(1)).unwrap()).unwrap();
    assert_eq!(response["id"], "confirm-me");
    assert_eq!(response["result"]["type"], "plugin_enabled", "{response}");
    assert!(server.app.state.installed_plugins["example.confirm"].enabled);
    assert!(server.app.current_api_confirmation().is_none());

    // A second answer for the same id has nothing left to run.
    assert!(answer(&mut server, SHELL_CLIENT, id, true));
    let _ = fs::remove_dir_all(root);
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_without_answer_times_out_as_not_confirmed() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let response_rx = socket_request(&mut server, plugin_enable("example.confirm"));

    assert!(!server.app.expire_api_confirmations(Instant::now()));
    assert!(response_rx.try_recv().is_err());
    assert!(server
        .app
        .expire_api_confirmations(Instant::now() + Duration::from_secs(121)));

    let (code, message) = error_of(&response_rx.recv_timeout(Duration::from_secs(1)).unwrap());
    assert_eq!(code, "not_confirmed");
    assert_eq!(message, "not confirmed");
    assert!(server.app.current_api_confirmation().is_none());
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_without_attached_client_is_rejected() {
    let mut server = test_headless_server();

    let response_rx = socket_request(&mut server, integration_install());

    let (code, message) = error_of(&response_rx.recv_timeout(Duration::from_secs(1)).unwrap());
    assert_eq!(code, "needs_confirmation");
    assert_eq!(message, "needs confirmation in an attached h0x client");
    assert!(server.app.current_api_confirmation().is_none());
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_plugin_link_prompt_names_plugin_and_path_and_bad_paths_fail_without_prompt() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let root = temp_plugin_root("link");
    let path = root.display().to_string();

    let response_rx = socket_request(
        &mut server,
        api::schema::Method::PluginLink(api::schema::PluginLinkParams {
            path: path.clone(),
            enabled: true,
            source: None,
        }),
    );
    assert!(response_rx.try_recv().is_err(), "request must wait");
    assert_eq!(
        server.app.current_api_confirmation().unwrap().prompt,
        format!("Link plugin example.confirm from {path}")
    );
    let id = pending_id(&server);
    assert!(answer(&mut server, SHELL_CLIENT, id, false));
    assert_eq!(
        error_of(&response_rx.recv_timeout(Duration::from_secs(1)).unwrap()).0,
        "denied_by_user"
    );
    assert!(server.app.state.installed_plugins.is_empty());

    let missing = socket_request(
        &mut server,
        api::schema::Method::PluginLink(api::schema::PluginLinkParams {
            path: root.join("missing").display().to_string(),
            enabled: true,
            source: None,
        }),
    );
    let (code, _) = error_of(&missing.recv_timeout(Duration::from_secs(1)).unwrap());
    assert_ne!(code, "denied_by_user");
    assert!(server.app.current_api_confirmation().is_none());
    let _ = fs::remove_dir_all(root);
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_answer_from_a_non_shell_client_is_ignored() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let response_rx = socket_request(&mut server, integration_install());
    let id = pending_id(&server);

    assert!(!answer(&mut server, SHELL_CLIENT + 1, id, true));

    assert!(response_rx.try_recv().is_err());
    assert_eq!(pending_id(&server), id);
    assert!(server
        .app
        .expire_api_confirmations(Instant::now() + Duration::from_secs(121)));
    shutdown_test_runtimes(&mut server);
}

#[tokio::test]
async fn api_confirmations_queue_in_order_and_each_answer_targets_its_own_request() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let first = socket_request(&mut server, integration_install());
    let first_id = pending_id(&server);
    let second = socket_request(&mut server, plugin_enable("example.other"));

    assert_eq!(pending_id(&server), first_id);
    assert!(answer(&mut server, SHELL_CLIENT, first_id, false));
    assert_eq!(
        error_of(&first.recv_timeout(Duration::from_secs(1)).unwrap()).0,
        "denied_by_user"
    );
    assert!(second.try_recv().is_err());
    let second_id = pending_id(&server);
    assert_ne!(second_id, first_id);

    // Allow runs the request; this plugin is not installed, so the handler's own error returns.
    assert!(answer(&mut server, SHELL_CLIENT, second_id, true));
    assert_eq!(
        error_of(&second.recv_timeout(Duration::from_secs(1)).unwrap()).0,
        "plugin_not_found"
    );
    shutdown_test_runtimes(&mut server);
}

#[tokio::test]
async fn client_shell_plugin_enable_runs_without_api_confirmation() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let root = install_disabled_plugin(&mut server);
    let (respond_to, response_rx) = std::sync::mpsc::channel();

    server.handle_client_shell_api_request(
        SHELL_CLIENT,
        api::ApiRequestMessage {
            request: api::schema::Request {
                id: "confirm-me".into(),
                method: plugin_enable("example.confirm"),
            },
            respond_to,
            response_write_complete: None,
        },
    );

    let response: serde_json::Value =
        serde_json::from_str(&response_rx.recv_timeout(Duration::from_secs(1)).unwrap()).unwrap();
    assert_eq!(response["result"]["type"], "plugin_enabled", "{response}");
    assert!(server.app.current_api_confirmation().is_none());
    let _ = fs::remove_dir_all(root);
    shutdown_test_runtimes(&mut server);
}
