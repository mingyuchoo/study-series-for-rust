use loco_rs::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub index_type: String,
    pub indicator_name: String,
    pub indicator_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct AzureMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct AzureChatRequest {
    messages: Vec<AzureMessage>,
    max_completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AzureChatResponse {
    choices: Vec<AzureChoice>,
}

#[derive(Debug, Deserialize)]
struct AzureChoice {
    message: AzureResponseMessage,
}

#[derive(Debug, Deserialize)]
struct AzureResponseMessage {
    #[serde(default)]
    content: Option<String>,
    // o1 모델 등에서 사용되는 필드
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuggestedIndex {
    pub name: String,
    pub description: String,
    pub target_value: Option<f64>,
    pub weight: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: String,
    pub suggestions: Vec<SuggestedIndex>,
}

fn get_system_prompt(index_type: &str) -> String {
    let index_type_korean = match index_type {
        "input" => "입력지표 (Input Index) - 투입되는 자원, 인력, 예산 등",
        "process" => "과정지표 (Process Index) - 진행 과정, 활동, 프로세스 등",
        "output" => "산출지표 (Output Index) - 직접적인 결과물, 생산량 등",
        "outcome" => "결과지표 (Outcome Index) - 최종 성과, 영향력, 효과 등",
        _ => "지표",
    };

    format!(
        r#"당신은 성과지표 관리 전문가입니다. 사용자가 {}를 생성하는 것을 도와주세요.

응답은 반드시 다음 JSON 형식으로만 제공하세요:
{{
  "message": "사용자에게 보여줄 친절한 설명 메시지",
  "suggestions": [
    {{
      "name": "지표명",
      "description": "지표에 대한 상세 설명",
      "target_value": 100.0,
      "weight": 1.0
    }}
  ]
}}

지침:
1. 지표명은 명확하고 측정 가능해야 합니다
2. 설명은 지표의 목적과 측정 방법을 포함해야 합니다
3. target_value는 합리적인 목표값을 제안하세요 (숫자 또는 null)
4. weight는 0.1~5.0 사이의 가중치입니다 (기본값 1.0)
5. 최대 3개의 추천을 제공하세요
6. 반드시 유효한 JSON만 출력하세요. 다른 텍스트는 포함하지 마세요."#,
        index_type_korean
    )
}

async fn chat(State(_ctx): State<AppContext>, Json(params): Json<ChatRequest>) -> Result<Response> {
    let api_key = std::env::var("AZURE_OPENAI_API_KEY")
        .map_err(|_| Error::string("AZURE_OPENAI_API_KEY 환경변수가 설정되지 않았습니다"))?;
    let endpoint = std::env::var("AZURE_OPENAI_ENDPOINT")
        .map_err(|_| Error::string("AZURE_OPENAI_ENDPOINT 환경변수가 설정되지 않았습니다"))?;
    let api_version = std::env::var("AZURE_OPENAI_API_VERSION")
        .map_err(|_| Error::string("AZURE_OPENAI_API_VERSION 환경변수가 설정되지 않았습니다"))?;
    let deployment_name = std::env::var("AZURE_OPENAI_DEPLOYMENT_NAME").map_err(|_| {
        Error::string("AZURE_OPENAI_DEPLOYMENT_NAME 환경변수가 설정되지 않았습니다")
    })?;

    let client = Client::new();

    let system_prompt = get_system_prompt(&params.index_type);
    let user_message = format!(
        "성과지표 '{}' ({})에 대한 {}를 추천해주세요.\n\n사용자 요청: {}",
        params.indicator_name,
        params
            .indicator_description
            .as_deref()
            .unwrap_or("설명 없음"),
        match params.index_type.as_str() {
            "input" => "입력지표",
            "process" => "과정지표",
            "output" => "산출지표",
            "outcome" => "결과지표",
            _ => "지표",
        },
        params.message
    );

    let request_body = AzureChatRequest {
        messages: vec![
            AzureMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            AzureMessage {
                role: "user".to_string(),
                content: user_message,
            },
        ],
        max_completion_tokens: 4000,
    };

    let url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version={}",
        endpoint, deployment_name, api_version
    );

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("api-key", &api_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| Error::string(&format!("Azure OpenAI 요청 실패: {}", e)))?;

    // 응답 본문을 텍스트로 먼저 받아서 로깅
    let response_text = response
        .text()
        .await
        .map_err(|e| Error::string(&format!("응답 읽기 실패: {}", e)))?;

    tracing::info!("Azure OpenAI 원시 응답: {}", response_text);

    let azure_response: AzureChatResponse = serde_json::from_str(&response_text)
        .map_err(|e| Error::string(&format!("응답 파싱 실패: {}, 원본: {}", e, response_text)))?;

    let content = azure_response
        .choices
        .first()
        .and_then(|c| {
            // content가 있으면 사용, 없으면 reasoning_content 사용
            c.message
                .content
                .clone()
                .or_else(|| c.message.reasoning_content.clone())
        })
        .unwrap_or_default();

    tracing::info!("추출된 content: '{}'", content);
    if content.is_empty() {
        tracing::warn!("content가 비어있음! 전체 응답: {}", response_text);
    }

    // JSON 추출 함수
    fn extract_json(text: &str) -> Option<String> {
        let text = text.trim();

        // 1. 마크다운 코드 블록 제거 시도
        let without_markdown = text
            .strip_prefix("```json")
            .or_else(|| text.strip_prefix("```"))
            .and_then(|s| s.strip_suffix("```"))
            .map(|s| s.trim().to_string());

        if let Some(json_str) = without_markdown {
            return Some(json_str);
        }

        // 2. 첫 번째 '{' 부터 마지막 '}' 까지 추출
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if end > start {
                    return Some(text[start..=end].to_string());
                }
            }
        }

        None
    }

    let clean_content = extract_json(&content).unwrap_or_else(|| content.clone());
    tracing::debug!("AI 원본 응답: {}", content);
    tracing::debug!("정제된 JSON: {}", clean_content);

    // JSON 파싱 시도
    let chat_response: ChatResponse = serde_json::from_str(&clean_content).unwrap_or_else(|e| {
        tracing::warn!("JSON 파싱 실패: {}, 원본: {}", e, clean_content);
        // JSON 파싱 실패 시 기본 응답
        ChatResponse {
            message: content,
            suggestions: vec![],
        }
    });

    format::json(chat_response)
}

pub fn routes() -> Routes {
    Routes::new().prefix("/api/ai").add("/chat", post(chat))
}
