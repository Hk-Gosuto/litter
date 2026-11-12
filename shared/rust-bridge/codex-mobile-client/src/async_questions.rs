//! Desktop-compatible async question identities and reply envelopes.

use crate::types::{
    PendingUserInputAnswer, PendingUserInputOption, PendingUserInputQuestion,
    PendingUserInputRequest,
};
use codex_app_server_protocol::{AsyncUserInputQuestion, UserInput};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Reply {
    pub question_item_id: String,
    pub question: String,
    pub answer: String,
}

pub(crate) fn parse(text: &str) -> Option<Vec<Reply>> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Replies {
        Many(Vec<Reply>),
        One(Reply),
    }
    let text = text.trim();
    let text = if text.starts_with("# Context from my IDE setup:\n") {
        text.rsplit_once("\n## My request for Codex:\n")?.1.trim()
    } else {
        text
    };
    let json = text
        .strip_prefix("<send_user_message_question_reply>")?
        .strip_suffix("</send_user_message_question_reply>")?;
    let replies = match serde_json::from_str::<Replies>(json).ok()? {
        Replies::Many(replies) => replies,
        Replies::One(reply) => vec![reply],
    };
    (!replies.is_empty()).then_some(replies)
}

pub(crate) fn parse_input(input: &[UserInput]) -> Option<Vec<Reply>> {
    let mut content = input
        .iter()
        .filter(|item| !matches!(item, UserInput::Skill { .. } | UserInput::Mention { .. }));
    let UserInput::Text { text, .. } = content.next()? else {
        return None;
    };
    if content.next().is_some() {
        return None;
    }
    parse(text)
}

pub(crate) fn questions(
    item_id: &str,
    questions: &[AsyncUserInputQuestion],
) -> Vec<PendingUserInputQuestion> {
    questions
        .iter()
        .enumerate()
        .filter(|(_, q)| !q.title.trim().is_empty())
        .map(|(index, q)| PendingUserInputQuestion {
            id: serde_json::json!(["request_user_input_async", item_id, index]).to_string(),
            header: None,
            question: q.title.clone(),
            is_other_allowed: true,
            is_secret: false,
            options: q
                .options
                .iter()
                .flatten()
                .take(32)
                .filter(|label| label.len() <= 512 && !label.trim().is_empty())
                .map(|label| PendingUserInputOption {
                    label: label.clone(),
                    description: None,
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn reply_text(
    request: &PendingUserInputRequest,
    answers: &[PendingUserInputAnswer],
) -> Result<String, String> {
    let replies = request
        .questions
        .iter()
        .map(|q| {
            let answer = answers
                .iter()
                .find(|a| a.question_id == q.id)
                .map(|a| {
                    a.answers
                        .iter()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "Enter an answer before sending".to_string())?;
            let end = q.question.floor_char_boundary(q.question.len().min(512));
            Ok(Reply {
                question_item_id: q.id.clone(),
                question: q.question[..end].replace(['\n', '\r'], " "),
                answer,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let text = format!(
        "<send_user_message_question_reply>\n{}\n</send_user_message_question_reply>",
        serde_json::to_string(&replies).map_err(|e| e.to_string())?
    );
    if text.chars().count() > codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS {
        return Err("Answer too long; shorten it before sending".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn async_question_replies_require_a_complete_valid_envelope() {
        let reply = r#"{"questionItemId":"id","question":"Which?","answer":"A"}"#;
        for body in [reply.to_string(), format!("[{reply}]")] {
            let text = format!(
                "<send_user_message_question_reply>{body}</send_user_message_question_reply>"
            );
            assert_eq!(parse(&text).unwrap()[0].answer, "A");
            assert_eq!(
                parse(&format!(
                    "# Context from my IDE setup:\ncontext\n## My request for Codex:\n{text}"
                ))
                .unwrap()[0]
                    .question_item_id,
                "id"
            );
            assert!(parse(&format!("quoted: {text}")).is_none());
            assert!(parse(&format!("{text} trailing")).is_none());
        }
        assert!(
            parse("<send_user_message_question_reply>[null]</send_user_message_question_reply>")
                .is_none()
        );
    }

    #[test]
    fn async_question_identity_matches_desktop_and_keeps_freeform_answers_literal() {
        let questions = questions(
            "item",
            &[AsyncUserInputQuestion {
                title: "Which?".into(),
                options: Some(vec!["A".into()]),
            }],
        );
        assert_eq!(questions[0].id, r#"["request_user_input_async","item",0]"#);
        let request: PendingUserInputRequest = serde_json::from_value(serde_json::json!({
            "id":"r", "serverId":"s", "threadId":"t", "turnId":"turn", "itemId":"item", "questions":questions, "isAsync":true
        })).unwrap();
        let text = reply_text(
            &request,
            &[PendingUserInputAnswer {
                question_id: request.questions[0].id.clone(),
                answers: vec!["/diff\n!echo <tag>".into()],
            }],
        )
        .unwrap();
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed[0].answer, "/diff\n!echo <tag>");
        assert!(!text.contains("user_note:"));
        assert!(reply_text(&request, &[]).is_err());
        let input = vec![UserInput::Text {
            text,
            text_elements: Vec::new(),
        }];
        assert!(parse_input(&input).is_some());
        let mut mixed = input;
        mixed.push(UserInput::Text {
            text: "extra".into(),
            text_elements: Vec::new(),
        });
        assert!(parse_input(&mixed).is_none());
    }
}
