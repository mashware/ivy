use super::{Account, AllowanceWindow, Bucket, Failure, Observation};
use serde_json::Value;

const MAX_BUCKETS: usize = 64;

impl Account {
    pub(super) fn from_response(response: Value) -> Result<Self, Failure> {
        let session = text(&response["activeSessionId"])?;
        let entries = response["sessions"]
            .as_array()
            .ok_or(Failure::Unavailable)?;
        let mut matching = entries
            .iter()
            .filter(|entry| entry["sessionId"].as_str() == Some(session));
        let active = matching.next().ok_or(Failure::Unavailable)?;
        if matching.next().is_some() || active["isActive"] != true {
            return Err(Failure::Unavailable);
        }
        let user = text(&active["userId"])?;
        let workspace = text(&active["selectedWorkspaceAccountId"])?;
        let account_label = optional_text(&active["email"])?
            .map_or_else(|| format!("Codex user {user}"), str::to_owned);
        let name = active["workspaces"]
            .as_array()
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| entry["accountId"].as_str() == Some(workspace))
            })
            .and_then(|entry| text(&entry["name"]).ok())
            .unwrap_or(workspace);
        Ok(Self {
            session: session.into(),
            user: user.into(),
            workspace: workspace.into(),
            label: format!("{account_label} · {name}"),
        })
    }
}

pub(super) fn bind_observation(
    before: Account,
    after: Account,
    result: Result<Value, i64>,
    observed_at: i64,
) -> Result<Observation, Failure> {
    if !before.matches(&after) {
        return Err(Failure::Unavailable);
    }
    let response = result.map_err(|code| match code {
        -32601 => Failure::Unsupported,
        _ => Failure::Unavailable,
    })?;
    // In the admitted client this field, including false, survives only when the backend's
    // account AND user match active auth. accountId alone would accept another user in the workspace.
    if response["accountId"].as_str() != Some(after.workspace.as_str())
        || !response["ordinaryUsageAllowed"].is_boolean()
    {
        return Err(Failure::Unavailable);
    }
    let buckets = parse_buckets(&response).map_err(|_| Failure::SameAccount(after.clone()))?;
    Ok(Observation {
        account: after,
        observed_at,
        buckets,
    })
}

fn parse_buckets(response: &Value) -> Result<Vec<Bucket>, Failure> {
    let buckets = &response["rateLimitsByLimitId"];
    if buckets.is_null() {
        let bucket = &response["rateLimits"];
        if !bucket.is_object() {
            return Err(Failure::Unavailable);
        }
        let id = optional_text(&bucket["limitId"])?.unwrap_or("codex");
        return Ok(vec![parse_bucket(id, bucket)?]);
    }
    let buckets = buckets
        .as_object()
        .filter(|map| map.len() <= MAX_BUCKETS)
        .ok_or(Failure::Unavailable)?;
    buckets
        .iter()
        .map(|(id, value)| parse_bucket(id, value))
        .collect()
}

fn parse_bucket(id: &str, value: &Value) -> Result<Bucket, Failure> {
    if !valid_text(id)
        || !value.is_object()
        || optional_text(&value["limitId"])?.is_some_and(|declared| declared != id)
    {
        return Err(Failure::Unavailable);
    }
    let label = optional_text(&value["limitName"])?
        .filter(|name| *name != id)
        .map_or_else(|| id.to_owned(), |name| format!("{name} ({id})"));
    Ok(Bucket {
        label,
        primary: parse_window(&value["primary"])?,
        secondary: parse_window(&value["secondary"])?,
    })
}

fn parse_window(value: &Value) -> Result<Option<AllowanceWindow>, Failure> {
    if value.is_null() {
        return Ok(None);
    }
    if !value.is_object() {
        return Err(Failure::Unavailable);
    }
    let used = optional(&value["usedPercent"], |value| {
        value.as_f64().filter(|used| (0.0..=100.0).contains(used))
    })?;
    let minutes = optional(&value["windowDurationMins"], |value| {
        value.as_i64().filter(|minutes| *minutes > 0)
    })?;
    let resets_at = optional(&value["resetsAt"], |value| {
        value.as_i64().filter(|seconds| {
            *seconds > 0 && chrono::DateTime::from_timestamp(*seconds, 0).is_some()
        })
    })?;
    Ok(Some(AllowanceWindow {
        used,
        minutes,
        resets_at,
    }))
}

fn optional<T>(
    value: &Value,
    parse: impl FnOnce(&Value) -> Option<T>,
) -> Result<Option<T>, Failure> {
    if value.is_null() {
        Ok(None)
    } else {
        parse(value).map(Some).ok_or(Failure::Unavailable)
    }
}

fn text(value: &Value) -> Result<&str, Failure> {
    value
        .as_str()
        .filter(|value| valid_text(value))
        .ok_or(Failure::Unavailable)
}

fn optional_text(value: &Value) -> Result<Option<&str>, Failure> {
    if value.is_null() {
        Ok(None)
    } else {
        text(value).map(Some)
    }
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn account(user: &str) -> Account {
        Account {
            session: user.into(),
            user: user.into(),
            workspace: "shared".into(),
            label: "Same displayed account".into(),
        }
    }

    fn quotas() -> Value {
        json!({
            "accountId": "shared",
            "ordinaryUsageAllowed": false,
            "rateLimits": {"limitId": "codex", "primary": {"usedPercent": 99}},
            "rateLimitsByLimitId": {
                "codex": {"limitId": "codex", "primary": {"usedPercent": 0}},
                "other": {"limitId": "other", "limitName": "Other", "secondary": {"resetsAt": 1800000000}}
            }
        })
    }

    #[test]
    fn preserves_bucket_identity_and_distinguishes_zero_from_unreported() {
        let snapshot = bind_observation(account("a"), account("a"), Ok(quotas()), 1729).unwrap();
        assert_eq!(snapshot.observed_at, 1729);
        assert_eq!(snapshot.buckets.len(), 2);
        assert_eq!(
            snapshot.buckets[0].primary.as_ref().unwrap().used,
            Some(0.0)
        );
        assert_eq!(snapshot.buckets[1].label, "Other (other)");
        assert_eq!(snapshot.buckets[1].secondary.as_ref().unwrap().used, None);
    }

    #[test]
    fn rejects_changed_user_and_unbound_quota_even_when_workspace_matches() {
        assert!(matches!(
            bind_observation(account("a"), account("b"), Ok(quotas()), 1),
            Err(Failure::Unavailable)
        ));
        let mut unknown = quotas();
        unknown["ordinaryUsageAllowed"] = Value::Null;
        assert!(matches!(
            bind_observation(account("a"), account("a"), Ok(unknown), 1),
            Err(Failure::Unavailable)
        ));
        assert!(matches!(
            bind_observation(account("a"), account("a"), Err(-32600), 1),
            Err(Failure::Unavailable)
        ));
    }

    #[test]
    fn rejects_bad_ranges_without_replacing_a_previous_observation() {
        let mut invalid = quotas();
        invalid["rateLimitsByLimitId"]["codex"]["primary"]["usedPercent"] = json!(101);
        assert!(matches!(
            bind_observation(account("a"), account("a"), Ok(invalid), 1),
            Err(Failure::SameAccount(_))
        ));
    }

    #[test]
    fn fallback_bucket_is_used_only_when_the_map_is_absent() {
        let mut response = quotas();
        response
            .as_object_mut()
            .unwrap()
            .remove("rateLimitsByLimitId");
        let parsed = parse_buckets(&response).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].primary.as_ref().unwrap().used, Some(99.0));
        response["rateLimitsByLimitId"] = json!({});
        assert!(parse_buckets(&response).unwrap().is_empty());
    }
}
