use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_app_server_protocol::RateLimitSnapshot;
use codex_app_server_protocol::RateLimitWindow;
use codex_app_server_protocol::StoredAccount;

pub(super) fn account_summary_lines(account: &StoredAccount) -> (String, Option<String>) {
    let active = if account.active { "*" } else { " " };
    let account_label = account.email.as_deref().unwrap_or(&account.label);
    let title = format!(
        "{active} {} [{}] {account_label}",
        account.account_id, account.auth_mode
    );
    let details = account
        .rate_limits
        .as_ref()
        .and_then(rate_limit_summary)
        .or_else(|| {
            account
                .active
                .then(|| "Usage stats unavailable.".to_string())
        });

    (title, details)
}

fn rate_limit_summary(snapshot: &RateLimitSnapshot) -> Option<String> {
    let captured_at = Local::now();
    let primary = snapshot
        .primary
        .as_ref()
        .map(|window| window_summary("5h", window, captured_at));
    let secondary = snapshot
        .secondary
        .as_ref()
        .map(|window| window_summary("weekly", window, captured_at));

    match (primary, secondary) {
        (Some(primary), Some(secondary)) => Some(format!("{primary}; {secondary}")),
        (Some(primary), None) => Some(primary),
        (None, Some(secondary)) => Some(secondary),
        (None, None) => None,
    }
}

fn window_summary(label: &str, window: &RateLimitWindow, captured_at: DateTime<Local>) -> String {
    let percent_remaining = (100.0 - f64::from(window.used_percent)).clamp(0.0, 100.0);
    let mut summary = format!("{label} {percent_remaining:.0}% left");
    if let Some(resets_at) = formatted_reset(window, captured_at) {
        summary.push_str(&format!(" (resets {resets_at})"));
    }
    summary
}

fn formatted_reset(window: &RateLimitWindow, captured_at: DateTime<Local>) -> Option<String> {
    window
        .resets_at
        .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
        .map(|dt| reset_duration_label(dt.with_timezone(&Local), captured_at))
}

fn reset_duration_label(dt: DateTime<Local>, captured_at: DateTime<Local>) -> String {
    let seconds = dt.timestamp().saturating_sub(captured_at.timestamp());
    if seconds <= 0 {
        return "now".to_string();
    }

    let minutes = (seconds + 59) / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }

    let hours = minutes / 60;
    let remaining_minutes = minutes % 60;
    if hours < 24 {
        if remaining_minutes == 0 {
            return format!("{hours}h");
        }
        return format!("{hours}h {remaining_minutes}m");
    }

    let days = hours / 24;
    let remaining_hours = hours % 24;
    if remaining_hours == 0 {
        format!("{days}d")
    } else {
        format!("{days}d {remaining_hours}h")
    }
}

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::AuthMode;
    use codex_app_server_protocol::RateLimitWindow;

    use super::*;

    #[test]
    fn account_summary_includes_active_rate_limits() {
        let account = StoredAccount {
            account_id: "acct-1".to_string(),
            label: "Work".to_string(),
            auth_mode: AuthMode::Chatgpt,
            email: Some("work@example.com".to_string()),
            plan_type: None,
            active: true,
            rate_limits: Some(RateLimitSnapshot {
                limit_id: Some("codex".to_string()),
                limit_name: Some("Codex".to_string()),
                primary: Some(RateLimitWindow {
                    used_percent: 42,
                    window_duration_mins: Some(300),
                    resets_at: None,
                }),
                secondary: Some(RateLimitWindow {
                    used_percent: 88,
                    window_duration_mins: Some(10080),
                    resets_at: None,
                }),
                credits: None,
                plan_type: None,
                rate_limit_reached_type: None,
            }),
        };

        let (title, details) = account_summary_lines(&account);

        assert_eq!(title, "* acct-1 [Chatgpt] work@example.com");
        assert_eq!(details, Some("5h 58% left; weekly 12% left".to_string()));
    }
}
