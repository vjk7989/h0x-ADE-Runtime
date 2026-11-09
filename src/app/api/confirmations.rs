//! Local API requests that only run after the user allows them in an attached client (ADR-049).

use std::time::{Duration, Instant};

use crate::api::schema::{Method, Request};
use crate::api::ApiRequestMessage;
use crate::app::App;

use super::responses::encode_error;

const API_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(120);
/// After a deny, the same program (by executable) is refused for this long.
const API_CONFIRMATION_DENY_COOLDOWN: Duration = Duration::from_secs(300);
/// Requests beyond this many waiting confirmations are refused at once.
const API_CONFIRMATION_MAX_PENDING: usize = 4;
const UNKNOWN_CALLER: &str = "an unidentified program";

pub(crate) struct PendingApiConfirmation {
    id: u64,
    prompt: String,
    caller: String,
    msg: ApiRequestMessage,
    deadline: Instant,
}

/// Unicode category Cf characters that can reorder or hide text (bidi controls, zero-width
/// characters, tags, ...).
fn is_format_char(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x2029
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

fn printable(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() || is_format_char(c) {
                '?'
            } else {
                c
            }
        })
        .collect()
}

fn caller_key(caller: Option<&str>) -> String {
    match caller.map(str::trim) {
        Some(caller) if !caller.is_empty() => printable(caller),
        _ => UNKNOWN_CALLER.to_owned(),
    }
}

/// The cooldown key for a caller description (`pid N (exe)`, optionally followed by parent
/// details): the executable, so a new process of the same program shares its predecessor's
/// cooldown. Anything without a program name shares one key.
fn denial_key(caller: &str) -> String {
    let exe = caller
        .strip_prefix("pid ")
        .and_then(|rest| rest.split_once(" ("))
        .map(|(_, rest)| rest.split("), parent pid ").next().unwrap_or(rest))
        .map(|name| name.trim_end_matches(')').trim())
        .filter(|name| !name.is_empty());
    exe.map_or_else(|| UNKNOWN_CALLER.to_owned(), str::to_owned)
}

impl App {
    /// The confirmation prompt for a socket request that installs hooks or plugin code,
    /// or the error response when such a request cannot run at all.
    pub(crate) fn api_confirmation_prompt(
        &self,
        request: &Request,
    ) -> Option<Result<String, String>> {
        match &request.method {
            Method::IntegrationInstall(params) => Some(Ok(format!(
                "Install the {} integration hooks",
                crate::integration::integration_target_label(params.target)
            ))),
            Method::PluginLink(params) if params.path.trim().is_empty() => Some(Err(encode_error(
                request.id.clone(),
                "invalid_params",
                "plugin path is empty",
            ))),
            Method::PluginLink(params) => Some(
                match super::plugins::load_plugin_manifest(&params.path, params.enabled) {
                    Ok(plugin) => Ok(format!(
                        "Link plugin {} from {}",
                        plugin.plugin_id,
                        printable(&params.path)
                    )),
                    Err((code, message)) => Err(encode_error(request.id.clone(), code, message)),
                },
            ),
            Method::PluginEnable(params) if params.plugin_id.trim().is_empty() => Some(Err(
                encode_error(request.id.clone(), "invalid_params", "plugin id is empty"),
            )),
            Method::PluginEnable(params) => Some(Ok(format!(
                "Enable plugin {}",
                printable(params.plugin_id.trim())
            ))),
            _ => None,
        }
    }

    /// Why a confirmation request is refused before it is queued, if it is: the caller was
    /// denied recently or too many requests already wait.
    pub(crate) fn api_confirmation_refusal(
        &mut self,
        caller: Option<&str>,
        now: Instant,
    ) -> Option<&'static str> {
        self.api_confirmation_denials.retain(|_, denied_at| {
            now.saturating_duration_since(*denied_at) < API_CONFIRMATION_DENY_COOLDOWN
        });
        if self
            .api_confirmation_denials
            .contains_key(&denial_key(&caller_key(caller)))
        {
            return Some("denied recently; try again later");
        }
        (self.api_confirmations.len() >= API_CONFIRMATION_MAX_PENDING)
            .then_some("too many confirmations are waiting")
    }

    pub(crate) fn queue_api_confirmation(
        &mut self,
        prompt: String,
        msg: ApiRequestMessage,
        now: Instant,
    ) {
        let id = self.next_api_confirmation_id;
        self.next_api_confirmation_id = id.wrapping_add(1);
        self.api_confirmations.push_back(PendingApiConfirmation {
            id,
            prompt,
            caller: caller_key(msg.caller.as_deref()),
            msg,
            deadline: now + API_CONFIRMATION_TIMEOUT,
        });
    }

    /// The confirmation shown to attached clients, oldest first.
    pub(crate) fn current_api_confirmation(
        &self,
    ) -> Option<crate::protocol::ClientShellApiConfirmation> {
        self.api_confirmations
            .front()
            .map(|pending| crate::protocol::ClientShellApiConfirmation {
                id: pending.id,
                prompt: pending.prompt.clone(),
                caller: pending.caller.clone(),
            })
    }

    /// Deny answers the caller with `denied by user`; Allow hands the request back for dispatch.
    pub(crate) fn answer_api_confirmation(
        &mut self,
        id: u64,
        allow: bool,
        now: Instant,
    ) -> Option<ApiRequestMessage> {
        let index = self
            .api_confirmations
            .iter()
            .position(|pending| pending.id == id)?;
        let pending = self.api_confirmations.remove(index)?;
        if allow {
            return Some(pending.msg);
        }
        self.api_confirmation_denials
            .insert(denial_key(&pending.caller), now);
        let _ = pending.msg.respond_to.send(encode_error(
            pending.msg.request.id,
            "denied_by_user",
            "denied by user",
        ));
        None
    }

    /// Answers every unanswered request past its deadline with `not confirmed`.
    pub(crate) fn expire_api_confirmations(&mut self, now: Instant) -> bool {
        let before = self.api_confirmations.len();
        let (expired, pending) = std::mem::take(&mut self.api_confirmations)
            .into_iter()
            .partition::<Vec<_>, _>(|pending| pending.deadline <= now);
        self.api_confirmations = pending.into();
        for pending in expired {
            let _ = pending.msg.respond_to.send(encode_error(
                pending.msg.request.id,
                "not_confirmed",
                "not confirmed",
            ));
        }
        self.api_confirmations.len() != before
    }
}
