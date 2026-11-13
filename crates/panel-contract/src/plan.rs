//! Project roadmap document shared by published plans and the Markdown fallback.

use crate::{Dropped, MAX_BYTES, Mark, Problem};

/// A project roadmap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub title: Option<String>,
    pub source: Option<String>,
    pub milestones: Vec<Milestone>,
    pub outside: Vec<Item>,
}

/// A roadmap milestone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Milestone {
    pub id: Option<String>,
    pub title: String,
    pub done_when: Option<String>,
    pub depends_on: Vec<String>,
    pub items: Vec<Item>,
}

/// One unit of work in a milestone or outside it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Item {
    pub id: Option<String>,
    pub text: String,
    pub mark: Option<Mark>,
    pub reference: Option<String>,
    pub link: Option<String>,
}

/// Reads a plan tolerantly, recording item marks outside the plan vocabulary.
pub fn parse_document_noting(text: &str) -> Result<(Plan, Dropped), Problem> {
    if text.len() as u64 > MAX_BYTES {
        return Err(Problem::TooBig);
    }
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|e| Problem::Invalid(e.to_string()))?;
    let root = raw
        .as_object()
        .ok_or_else(|| Problem::Invalid("not an object".into()))?;
    let mut dropped = Dropped::default();
    let milestones = root
        .get("milestones")
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| milestone(value, &mut dropped))
                .collect()
        })
        .unwrap_or_default();
    let outside = root
        .get("outside")
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| item(value, &mut dropped))
                .collect()
        })
        .unwrap_or_default();
    Ok((
        Plan {
            title: string(root.get("title")),
            source: string(root.get("source")),
            milestones,
            outside,
        },
        dropped,
    ))
}

fn milestone(raw: &serde_json::Value, dropped: &mut Dropped) -> Option<Milestone> {
    let object = raw.as_object()?;
    let items = object
        .get("items")
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| item(value, dropped))
                .collect()
        })
        .unwrap_or_default();
    let depends_on = object
        .get("depends_on")
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Some(Milestone {
        id: string(object.get("id")),
        title: string(object.get("title"))?,
        done_when: string(object.get("done_when")),
        depends_on,
        items,
    })
}

fn item(raw: &serde_json::Value, dropped: &mut Dropped) -> Option<Item> {
    let object = raw.as_object()?;
    let mark_value = object.get("mark");
    let mark = mark_value.and_then(|v| v.as_str()).and_then(mark);
    if let Some(value) = mark_value
        .filter(|_| mark.is_none())
        .filter(|v| !v.is_null())
    {
        dropped.note(crate::dropped::Vocabulary::Mark, value);
    }
    Some(Item {
        id: string(object.get("id")),
        text: string(object.get("text"))?,
        mark,
        reference: string(object.get("ref")),
        link: string(object.get("link")),
    })
}

fn string(value: Option<&serde_json::Value>) -> Option<String> {
    value.and_then(|v| v.as_str()).map(str::to_owned)
}

fn mark(value: &str) -> Option<Mark> {
    Some(match value {
        "done" => Mark::Done,
        "current" => Mark::Current,
        "pending" => Mark::Pending,
        "wait" => Mark::Wait,
        "block" => Mark::Block,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_fields_and_excludes_info_mark() {
        let input = r##"{"title":"Roadmap","source":"plan.md","milestones":[{"id":"M1","title":"Build","done_when":"shipped","depends_on":["M0"],"items":[{"id":"I1","text":"code","mark":"current","ref":"#1","link":"https://example.test"},{"text":"extra","mark":"info"}]}],"outside":[{"text":"cleanup","mark":"pending"}]}"##;
        let (plan, dropped) = parse_document_noting(input).unwrap();
        assert_eq!(plan.title.as_deref(), Some("Roadmap"));
        assert_eq!(plan.milestones[0].depends_on, ["M0"]);
        assert_eq!(plan.milestones[0].items[0].mark, Some(Mark::Current));
        assert_eq!(plan.milestones[0].items[0].reference.as_deref(), Some("#1"));
        assert_eq!(plan.milestones[0].items[1].mark, None);
        assert!(dropped.sentence().unwrap().contains("info"));
        assert_eq!(plan.outside[0].mark, Some(Mark::Pending));
    }

    #[test]
    fn malformed_entries_are_dropped_and_size_is_refused() {
        let (plan, _) = parse_document_noting(
            r#"{"milestones":[2,{"title":3},{"title":"ok","items":[null,{"text":"kept"}]}]}"#,
        )
        .unwrap();
        assert_eq!(plan.milestones.len(), 1);
        assert_eq!(plan.milestones[0].items.len(), 1);
        assert!(matches!(
            parse_document_noting(&" ".repeat(MAX_BYTES as usize + 1)),
            Err(Problem::TooBig)
        ));
    }
}
