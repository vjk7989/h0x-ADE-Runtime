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
    socket_request_from(server, method, None)
}

fn socket_request_from(
    server: &mut HeadlessServer,
    method: api::schema::Method,
    caller: Option<&str>,
) -> std::sync::mpsc::Receiver<String> {
    let (respond_to, response_rx) = std::sync::mpsc::channel();
    server.handle_api_request_with_shutdown_check(api::ApiRequestMessage {
        request: api::schema::Request {
            id: "confirm-me".into(),
            method,
        },
        respond_to,
        response_write_complete: None,
        caller: caller.map(str::to_owned),
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
            caller: None,
        },
    );

    let response: serde_json::Value =
        serde_json::from_str(&response_rx.recv_timeout(Duration::from_secs(1)).unwrap()).unwrap();
    assert_eq!(response["result"]["type"], "plugin_enabled", "{response}");
    assert!(server.app.current_api_confirmation().is_none());
    let _ = fs::remove_dir_all(root);
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_shows_the_requesting_process() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let _rx = socket_request_from(&mut server, integration_install(), Some("pid 42 (node)"));
    assert_eq!(
        server.app.current_api_confirmation().unwrap().caller,
        "pid 42 (node)"
    );
    shutdown_test_runtimes(&mut server);

    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let _rx = socket_request(&mut server, integration_install());
    assert_eq!(
        server.app.current_api_confirmation().unwrap().caller,
        "an unidentified program"
    );
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_after_a_deny_refuses_the_same_caller_only() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let first = socket_request_from(&mut server, integration_install(), Some("pid 1 (a)"));
    let id = pending_id(&server);
    assert!(answer(&mut server, SHELL_CLIENT, id, false));
    assert_eq!(
        error_of(&first.recv_timeout(Duration::from_secs(1)).unwrap()).0,
        "denied_by_user"
    );

    let again = socket_request_from(&mut server, integration_install(), Some("pid 1 (a)"));
    let (code, message) = error_of(&again.recv_timeout(Duration::from_secs(1)).unwrap());
    assert_eq!(code, "confirmation_refused");
    assert_eq!(message, "denied recently; try again later");
    assert!(server.app.current_api_confirmation().is_none());

    let other = socket_request_from(&mut server, integration_install(), Some("pid 2 (b)"));
    assert!(
        other.try_recv().is_err(),
        "another caller still waits for the user"
    );
    assert!(server.app.current_api_confirmation().is_some());
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_deny_cooldown_expires_at_five_minutes() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let _rx = socket_request_from(&mut server, integration_install(), Some("pid 1 (a)"));
    let id = pending_id(&server);
    let denied_at = Instant::now();
    assert!(server
        .app
        .answer_api_confirmation(id, false, denied_at)
        .is_none());

    let caller = Some("pid 1 (a)");
    let just_before = denied_at + Duration::from_secs(299);
    assert!(server
        .app
        .api_confirmation_refusal(caller, just_before)
        .is_some());
    let at_expiry = denied_at + Duration::from_secs(300);
    assert!(server
        .app
        .api_confirmation_refusal(caller, at_expiry)
        .is_none());
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_queue_is_capped_and_extra_requests_are_refused_at_once() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let waiting = (0..4)
        .map(|n| {
            let caller = format!("pid {n} (x)");
            socket_request_from(&mut server, integration_install(), Some(&caller))
        })
        .collect::<Vec<_>>();
    for rx in &waiting {
        assert!(rx.try_recv().is_err(), "within the cap the request waits");
    }
    let fifth = socket_request_from(&mut server, integration_install(), Some("pid 9 (x)"));
    let (code, message) = error_of(&fifth.recv_timeout(Duration::from_secs(1)).unwrap());
    assert_eq!(code, "confirmation_refused");
    assert_eq!(message, "too many confirmations are waiting");

    // Answering one frees a slot.
    let id = pending_id(&server);
    assert!(answer(&mut server, SHELL_CLIENT, id, false));
    let sixth = socket_request_from(&mut server, integration_install(), Some("pid 10 (x)"));
    assert!(sixth.try_recv().is_err());
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_prompt_strips_bidi_and_format_characters() {
    let server = test_headless_server();
    let request = api::schema::Request {
        id: "confirm-me".into(),
        method: plugin_enable(
            "evil\u{202e}gnp\u{200b}.\u{2066}x\u{2069}\u{feff}\u{e0041}\u{0007}ok",
        ),
    };
    let prompt = server
        .app
        .api_confirmation_prompt(&request)
        .unwrap()
        .unwrap();
    assert_eq!(prompt, "Enable plugin evil?gnp?.?x????ok");
}

fn enable_prompt(server: &HeadlessServer, plugin_id: &str) -> String {
    server
        .app
        .api_confirmation_prompt(&api::schema::Request {
            id: "confirm-me".into(),
            method: plugin_enable(plugin_id),
        })
        .unwrap()
        .unwrap()
}

#[test]
fn api_confirmation_prompt_replaces_each_listed_format_character_and_keeps_its_neighbours() {
    let server = test_headless_server();
    let format_chars = [
        0x00AD, 0x061C, 0x180E, 0x200B, 0x200C, 0x200D, 0x200E, 0x200F, 0x202A, 0x202B, 0x202C,
        0x202D, 0x202E, 0x2060, 0x2061, 0x2062, 0x2063, 0x2064, 0x2066, 0x2067, 0x2068, 0x2069,
        0xFEFF, 0xE0001, 0xE0020, 0xE0041, 0xE007F,
    ];
    for code in format_chars {
        let c = char::from_u32(code).unwrap();
        assert_eq!(
            enable_prompt(&server, &format!("a{c}b")),
            "Enable plugin a?b",
            "U+{code:04X}"
        );
    }
    // Printable neighbours of the replaced ranges stay as they are.
    for code in [0x200A, 0x2010, 0x202F, 0x2070, 0xE0080, 0x00E9] {
        let c = char::from_u32(code).unwrap();
        assert_eq!(
            enable_prompt(&server, &format!("a{c}b")),
            format!("Enable plugin a{c}b"),
            "U+{code:04X}"
        );
    }
    // Controls: C0, DEL and C1.
    assert_eq!(
        enable_prompt(&server, "a\u{1b}[31mb\u{7f}c\u{85}d"),
        "Enable plugin a?[31mb?c?d"
    );
    // An id that is empty after trimming must not panic.
    assert!(enable_prompt(&server, "   ").starts_with("Enable plugin"));
}

#[test]
fn api_confirmation_prompt_breaks_emoji_zwj_sequences() {
    // Current behaviour: U+200D is a format character, so a family emoji shows as its parts.
    let server = test_headless_server();
    assert_eq!(
        enable_prompt(&server, "team\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
        "Enable plugin team\u{1F468}?\u{1F469}?\u{1F467}"
    );
}

#[test]
fn api_confirmation_caller_is_shown_printable_with_windows_paths_intact() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let _rx = socket_request_from(
        &mut server,
        integration_install(),
        Some("pid 7 (C:\\Tools\\node.exe)\u{202e}exe.txt\u{200b}\u{1b}"),
    );
    assert_eq!(
        server.app.current_api_confirmation().unwrap().caller,
        "pid 7 (C:\\Tools\\node.exe)?exe.txt??"
    );
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_allow_and_timeout_do_not_start_the_deny_cooldown() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let caller = Some("pid 1 (a)");

    let _allowed = socket_request_from(&mut server, integration_install(), caller);
    let id = pending_id(&server);
    let now = Instant::now();
    // Allow hands the request back for dispatch; it is not run here.
    assert!(server.app.answer_api_confirmation(id, true, now).is_some());
    assert!(server.app.api_confirmation_refusal(caller, now).is_none());

    let timed_out = socket_request_from(&mut server, integration_install(), caller);
    let later = Instant::now() + Duration::from_secs(121);
    assert!(server.app.expire_api_confirmations(later));
    assert_eq!(
        error_of(&timed_out.recv_timeout(Duration::from_secs(1)).unwrap()).0,
        "not_confirmed"
    );
    assert!(server.app.api_confirmation_refusal(caller, later).is_none());

    // An unknown id answers nothing and denies nobody.
    assert!(server
        .app
        .answer_api_confirmation(999, false, later)
        .is_none());
    assert!(server.app.api_confirmation_refusal(caller, later).is_none());
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_deny_of_an_unidentified_caller_refuses_other_unidentified_callers() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let first = socket_request(&mut server, integration_install());
    let id = pending_id(&server);
    assert!(answer(&mut server, SHELL_CLIENT, id, false));
    assert_eq!(
        error_of(&first.recv_timeout(Duration::from_secs(1)).unwrap()).0,
        "denied_by_user"
    );

    let again = socket_request(&mut server, integration_install());
    assert_eq!(
        error_of(&again.recv_timeout(Duration::from_secs(1)).unwrap()).0,
        "confirmation_refused"
    );
    // A known caller is unaffected.
    let known = socket_request_from(&mut server, integration_install(), Some("pid 5 (a)"));
    assert!(known.try_recv().is_err());
    shutdown_test_runtimes(&mut server);
}

#[test]
fn api_confirmation_cooldown_is_checked_before_the_queue_cap() {
    let mut server = test_headless_server();
    attach_shell_client(&mut server);
    let _first = socket_request_from(&mut server, integration_install(), Some("pid 1 (a)"));
    let id = pending_id(&server);
    assert!(answer(&mut server, SHELL_CLIENT, id, false));
    let _waiting = (0..4)
        .map(|n| {
            let caller = format!("pid {} (x)", n + 10);
            socket_request_from(&mut server, integration_install(), Some(&caller))
        })
        .collect::<Vec<_>>();
    let denied = socket_request_from(&mut server, integration_install(), Some("pid 1 (a)"));
    assert_eq!(
        error_of(&denied.recv_timeout(Duration::from_secs(1)).unwrap()).1,
        "denied recently; try again later"
    );
    // The waiting requests are untouched by the refusals.
    assert_eq!(
        server.app.current_api_confirmation().unwrap().caller,
        "pid 10 (x)"
    );
    shutdown_test_runtimes(&mut server);
}
