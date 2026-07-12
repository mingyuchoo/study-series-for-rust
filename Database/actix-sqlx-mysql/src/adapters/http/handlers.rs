use super::models::{CreateMemberRequest,
                    ErrorResponse,
                    MemberResponse,
                    MembersResponse,
                    SuccessResponse,
                    UpdateMemberRequest};
use crate::application::MemberUseCase;
use actix_web::{HttpResponse,
                Responder,
                web};
use std::sync::Arc;

/// 헬스 체크 핸들러
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(SuccessResponse {
        message: "서버가 정상적으로 실행 중입니다.".to_string(),
    })
}

/// Member 생성 핸들러
pub async fn create_member(usecase: web::Data<Arc<MemberUseCase>>, req: web::Json<CreateMemberRequest>) -> impl Responder {
    match usecase.create_member(req.name.clone()).await {
        | Ok(member) => HttpResponse::Created().json(MemberResponse::from(member)),
        | Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

/// ID로 Member 조회 핸들러
pub async fn get_member(usecase: web::Data<Arc<MemberUseCase>>, path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();

    match usecase.get_member_by_id(&id).await {
        | Ok(Some(member)) => HttpResponse::Ok().json(MemberResponse::from(member)),
        | Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            error: "Member를 찾을 수 없습니다.".to_string(),
        }),
        | Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

/// 모든 Member 조회 핸들러
pub async fn get_all_members(usecase: web::Data<Arc<MemberUseCase>>) -> impl Responder {
    match usecase.get_all_members().await {
        | Ok(members) => {
            let total = members.len();
            let members: Vec<MemberResponse> = members.into_iter().map(MemberResponse::from).collect();
            HttpResponse::Ok().json(MembersResponse {
                members,
                total,
            })
        },
        | Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

/// Member 개수 조회 핸들러
pub async fn get_member_count(usecase: web::Data<Arc<MemberUseCase>>) -> impl Responder {
    match usecase.get_member_count().await {
        | Ok(count) => HttpResponse::Ok().json(serde_json::json!({ "count": count })),
        | Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

/// Member 업데이트 핸들러
pub async fn update_member(usecase: web::Data<Arc<MemberUseCase>>, path: web::Path<String>, req: web::Json<UpdateMemberRequest>) -> impl Responder {
    let id = path.into_inner();

    // 먼저 Member가 존재하는지 확인
    match usecase.get_member_by_id(&id).await {
        | Ok(Some(mut member)) => {
            member.name = req.name.clone();
            match usecase.update_member(member).await {
                | Ok(_) => HttpResponse::Ok().json(SuccessResponse {
                    message: "Member가 성공적으로 업데이트되었습니다.".to_string(),
                }),
                | Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
                    error: e.to_string(),
                }),
            }
        },
        | Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            error: "Member를 찾을 수 없습니다.".to_string(),
        }),
        | Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

/// Member 삭제 핸들러
pub async fn delete_member(usecase: web::Data<Arc<MemberUseCase>>, path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();

    match usecase.delete_member(&id).await {
        | Ok(_) => HttpResponse::Ok().json(SuccessResponse {
            message: "Member가 성공적으로 삭제되었습니다.".to_string(),
        }),
        | Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}
