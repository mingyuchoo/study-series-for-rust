use crate::domain::Member;
use serde::{Deserialize,
            Serialize};

/// Member 생성 요청 DTO
#[derive(Debug, Deserialize)]
pub struct CreateMemberRequest {
    pub name: String,
}

/// Member 업데이트 요청 DTO
#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub name: String,
}

/// Member 응답 DTO
#[derive(Debug, Serialize)]
pub struct MemberResponse {
    pub id: String,
    pub name: String,
}

impl From<Member> for MemberResponse {
    fn from(member: Member) -> Self {
        Self {
            id: member.id,
            name: member.name,
        }
    }
}

/// Member 목록 응답 DTO
#[derive(Debug, Serialize)]
pub struct MembersResponse {
    pub members: Vec<MemberResponse>,
    pub total: usize,
}

/// 에러 응답 DTO
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// 성공 응답 DTO
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub message: String,
}
