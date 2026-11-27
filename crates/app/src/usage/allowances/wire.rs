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
    response: Value,
    observed_at: i64,
) -> Result<Observation, Failure> {
    if !before.matches(&after) {
        return Err(Failure::Identity("Codex's account changed during the read."));
    }
    if response["accountId"].as_str() != Some(after.workspace.as_str()) {
        return Err(Failure::Identity("Codex answered for another workspace."));
    }
    // In 0.167.2 this field, including false, survives only when the backend's account AND user
    // match active auth; accountId alone would accept another user in the workspace. With no
    // release pinned it is the evidence every read must carry, and its absence is a refusal.
    // Residual risk (D132): its meaning as a witness was proven for 0.167.2 only; 0.172.0 was
    // seen to send it for a matching user, not to withhold it for another. A release that sends
    // it unconditionally would let another user's reading through this check; the sign-in source
    // (#283) does not depend on it and is preferred where it is available.
    if !response["ordinaryUsageAllowed"].is_boolean() {
        return Err(Failure::Identity(
            "Codex sent no identity witness (ordinaryUsageAllowed).",
        ));
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
        value
            .as_i64()
            .filter(|minutes| (1..=super::MAX_WINDOW / 60).contains(minutes))
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

    fn refusal(result: Result<Observation, Failure>) -> &'static str {
        match result {
            Err(Failure::Identity(reason)) => reason,
            other => panic!("a refusal was expected: {other:?}"),
        }
    }

    #[test]
    fn preserves_bucket_identity_and_distinguishes_zero_from_unreported() {
        let snapshot = bind_observation(account("a"), account("a"), quotas(), 1729).unwrap();
        assert_eq!(snapshot.observed_at, 1729);
        assert_eq!(snapshot.buckets.len(), 2);
        assert_eq!(
            snapshot.buckets[0].primary.as_ref().unwrap().used,
            Some(0.0)
        );
        assert_eq!(snapshot.buckets[1].label, "Other (other)");
        assert_eq!(snapshot.buckets[1].secondary.as_ref().unwrap().used, None);
    }

    /// The identity evidence is the gate, whatever the release: a changed user, another
    /// workspace, or a missing witness is refused with what was missing — never a reading. A
    /// witness of `false` is still a witness.
    #[test]
    fn refuses_a_changed_user_another_workspace_or_a_missing_witness_by_name() {
        assert_eq!(
            refusal(bind_observation(account("a"), account("b"), quotas(), 1)),
            "Codex's account changed during the read."
        );
        let mut elsewhere = quotas();
        elsewhere["accountId"] = json!("other");
        assert_eq!(
            refusal(bind_observation(account("a"), account("a"), elsewhere, 1)),
            "Codex answered for another workspace."
        );
        for missing in [Value::Null, json!("false")] {
            let mut unknown = quotas();
            unknown["ordinaryUsageAllowed"] = missing;
            assert_eq!(
                refusal(bind_observation(account("a"), account("a"), unknown, 1)),
                "Codex sent no identity witness (ordinaryUsageAllowed)."
            );
        }
        assert!(bind_observation(account("a"), account("a"), quotas(), 1).is_ok());
    }

    #[test]
    fn rejects_bad_ranges_without_replacing_a_previous_observation() {
        let mut invalid = quotas();
        invalid["rateLimitsByLimitId"]["codex"]["primary"]["usedPercent"] = json!(101);
        assert!(matches!(
            bind_observation(account("a"), account("a"), invalid, 1),
            Err(Failure::SameAccount(_))
        ));
        // A window longer than a year and a day is refused as out of range, not multiplied.
        for minutes in [i64::MAX, 366 * 24 * 60 + 1] {
            let mut absurd = quotas();
            absurd["rateLimitsByLimitId"]["codex"]["primary"]["windowDurationMins"] =
                json!(minutes);
            assert!(matches!(
                bind_observation(account("a"), account("a"), absurd, 1),
                Err(Failure::SameAccount(_))
            ));
        }
        let mut year = quotas();
        year["rateLimitsByLimitId"]["codex"]["primary"]["windowDurationMins"] =
            json!(366 * 24 * 60);
        assert!(bind_observation(account("a"), account("a"), year, 1).is_ok());
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
