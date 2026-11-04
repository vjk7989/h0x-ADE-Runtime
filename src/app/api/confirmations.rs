//! Local API requests that only run after the user allows them in an attached client (ADR-049).

use std::time::{Duration, Instant};

use crate::api::schema::{Method, Request};
use crate::api::ApiRequestMessage;
use crate::app::App;

use super::responses::encode_error;

const API_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) struct PendingApiConfirmation {
    id: u64,
    prompt: String,
    msg: ApiRequestMessage,
    deadline: Instant,
}

fn printable(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
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
            Method::PluginEnable(params) => Some(Ok(format!(
                "Enable plugin {}",
                printable(params.plugin_id.trim())
            ))),
            _ => None,
        }
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
            })
    }

    /// Deny answers the caller with `denied by user`; Allow hands the request back for dispatch.
    pub(crate) fn answer_api_confirmation(
        &mut self,
        id: u64,
        allow: bool,
    ) -> Option<ApiRequestMessage> {
        let index = self
            .api_confirmations
            .iter()
            .position(|pending| pending.id == id)?;
        let pending = self.api_confirmations.remove(index)?;
        if allow {
            return Some(pending.msg);
        }
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
