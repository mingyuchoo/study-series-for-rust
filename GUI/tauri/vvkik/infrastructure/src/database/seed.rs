//! 첫 실행 시 비어 있는 데이터베이스를 VVKIK 예시 계층으로 채운다.
//!
//! 멱등성: 항목이 하나라도 있으면 아무것도 하지 않으므로 앱을 여러 번
//! 실행해도 시드가 중복으로 들어가지 않는다.
//!
//! 시드 규모는 단계마다 부모 하나당 자식 둘(2 × 2 × 2 × 2 × 2)로,
//! Value 2 → Vision 4 → KRA 8 → IGT 16 → KPI 32, 총 62개 항목이다.

use domain::{DomainError,
             ItemKind,
             KpiAggregation,
             NewVvkikItem,
             VvkikItem,
             VvkikRepository};
use uuid::Uuid;

struct KpiSeed {
    title: &'static str,
    target_value: f64,
    unit: &'static str,
    aggregation: KpiAggregation,
}

struct IgtSeed {
    title: &'static str,
    kpis: [KpiSeed; 2],
}

struct KraSeed {
    title: &'static str,
    igts: [IgtSeed; 2],
}

struct VisionSeed {
    title: &'static str,
    description: &'static str,
    kras: [KraSeed; 2],
}

struct ValueSeed {
    title: &'static str,
    description: &'static str,
    visions: [VisionSeed; 2],
}

fn kpi(title: &'static str, target_value: f64, unit: &'static str, aggregation: KpiAggregation) -> KpiSeed {
    KpiSeed {
        title,
        target_value,
        unit,
        aggregation,
    }
}

fn igt(title: &'static str, kpis: [KpiSeed; 2]) -> IgtSeed {
    IgtSeed {
        title,
        kpis,
    }
}

fn kra(title: &'static str, igts: [IgtSeed; 2]) -> KraSeed {
    KraSeed {
        title,
        igts,
    }
}

fn vision(title: &'static str, description: &'static str, kras: [KraSeed; 2]) -> VisionSeed {
    VisionSeed {
        title,
        description,
        kras,
    }
}

fn seed_tree() -> [ValueSeed; 2] {
    use KpiAggregation::{Average,
                         Latest,
                         Sum};

    [
        ValueSeed {
            title: "경제적 자유",
            description: "돈이 아니라 시간을 기준으로 선택할 수 있는 삶",
            visions: [
                vision(
                    "3년 내 월 패시브 인컴 500만원 달성",
                    "노동 시간과 분리된 수입원을 구축한다",
                    [
                        kra(
                            "온라인 강의 사업",
                            [
                                igt(
                                    "신규 강의 콘텐츠 제작",
                                    [kpi("월 신규 강의 수", 2.0, "개", Sum), kpi("강의 완강률", 60.0, "%", Latest)],
                                ),
                                igt(
                                    "강의 런칭 마케팅",
                                    [kpi("월 신규 수강생 수", 100.0, "명", Sum), kpi("랜딩 페이지 전환율", 5.0, "%", Latest)],
                                ),
                            ],
                        ),
                        kra(
                            "투자 포트폴리오 운용",
                            [
                                igt(
                                    "배당주 포트폴리오 리밸런싱",
                                    [kpi("연 배당 수익률", 5.0, "%", Latest), kpi("월 평균 배당금", 50.0, "만원", Average)],
                                ),
                                igt(
                                    "부동산 임대 수익 관리",
                                    [kpi("월 임대 순수익", 200.0, "만원", Average), kpi("연 공실률", 5.0, "%", Latest)],
                                ),
                            ],
                        ),
                    ],
                ),
                vision(
                    "5년 내 순자산 10억 달성",
                    "수입 극대화와 지출 최적화를 병행한다",
                    [
                        kra(
                            "수입 극대화",
                            [
                                igt(
                                    "고단가 컨설팅 계약 수주",
                                    [kpi("분기 신규 계약 건수", 2.0, "건", Sum), kpi("평균 계약 단가", 1000.0, "만원", Average)],
                                ),
                                igt(
                                    "연봉 협상 및 이직 준비",
                                    [kpi("기술 면접 통과율", 70.0, "%", Latest), kpi("포트폴리오 완성도", 100.0, "%", Latest)],
                                ),
                            ],
                        ),
                        kra(
                            "지출 최적화",
                            [
                                igt(
                                    "고정비 절감 실행",
                                    [kpi("월 고정비 상한", 150.0, "만원", Latest), kpi("유지 구독 서비스 수", 5.0, "개", Latest)],
                                ),
                                igt(
                                    "예산 관리 시스템 구축",
                                    [kpi("월 예산 준수율", 90.0, "%", Average), kpi("월 저축률", 40.0, "%", Average)],
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        },
        ValueSeed {
            title: "건강과 성장",
            description: "오래 일하고 오래 배울 수 있는 몸과 머리를 유지한다",
            visions: [
                vision(
                    "활력 있는 몸 만들기",
                    "운동과 회복 루틴을 생활에 고정한다",
                    [
                        kra(
                            "근력 운동 루틴",
                            [
                                igt(
                                    "주 3회 웨이트 트레이닝",
                                    [kpi("주간 운동 횟수", 3.0, "회", Sum), kpi("3대 운동 합계 중량", 300.0, "kg", Latest)],
                                ),
                                igt(
                                    "식단 단백질 관리",
                                    [kpi("일일 단백질 섭취량", 120.0, "g", Average), kpi("체지방률", 15.0, "%", Latest)],
                                ),
                            ],
                        ),
                        kra(
                            "유산소·회복 관리",
                            [
                                igt(
                                    "주 2회 러닝",
                                    [kpi("주간 러닝 거리", 10.0, "km", Sum), kpi("5km 완주 기록", 25.0, "분", Latest)],
                                ),
                                igt(
                                    "수면 루틴 개선",
                                    [kpi("평균 수면 시간", 7.0, "시간", Average), kpi("취침 시간 준수율", 80.0, "%", Average)],
                                ),
                            ],
                        ),
                    ],
                ),
                vision(
                    "업계가 인정하는 전문가 되기",
                    "기술 역량과 영향력을 함께 키운다",
                    [
                        kra(
                            "기술 역량 강화",
                            [
                                igt(
                                    "Rust 사이드 프로젝트 완성",
                                    [kpi("월 커밋 수", 60.0, "회", Sum), kpi("프로젝트 완성률", 100.0, "%", Latest)],
                                ),
                                igt(
                                    "기술 블로그 운영",
                                    [kpi("월 포스팅 수", 4.0, "건", Sum), kpi("월 방문자 수", 1000.0, "명", Latest)],
                                ),
                            ],
                        ),
                        kra(
                            "네트워크·영향력 확대",
                            [
                                igt(
                                    "컨퍼런스 발표",
                                    [kpi("연간 발표 횟수", 2.0, "회", Sum), kpi("발표 자료 완성도", 100.0, "%", Latest)],
                                ),
                                igt(
                                    "커뮤니티 스터디 리딩",
                                    [kpi("월 스터디 진행 횟수", 4.0, "회", Sum), kpi("스터디 참여 인원", 10.0, "명", Latest)],
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        },
    ]
}

/// `metric`은 KPI 전용으로, (목표값, 단위, 집계 방식)이다. 현재값은
/// 0에서 시작한다.
async fn create<R>(
    repository: &R,
    kind: ItemKind,
    parent_id: Option<Uuid>,
    title: &str,
    description: Option<&str>,
    metric: Option<(f64, &str, KpiAggregation)>,
    position: i64,
) -> Result<Uuid, DomainError>
where
    R: VvkikRepository + ?Sized,
{
    let item = VvkikItem::new(NewVvkikItem {
        kind,
        parent_id,
        title: title.to_string(),
        description: description.map(str::to_string),
        target_value: metric.map(|(target, _, _)| target),
        current_value: metric.map(|_| 0.0),
        unit: metric.map(|(_, unit, _)| unit.to_string()),
        position,
        aggregation: metric.map(|(_, _, aggregation)| aggregation).unwrap_or_default(),
    });
    let id = item.id;
    repository.create_item(item).await?;
    Ok(id)
}

/// 데이터베이스가 비어 있으면 VVKIK 예시 계층(총 62개 항목)을 넣는다.
///
/// 시드를 넣었으면 `true`, 이미 데이터가 있어 건너뛰었으면 `false`를
/// 돌려준다.
pub async fn seed_if_empty<R>(repository: &R) -> Result<bool, DomainError>
where
    R: VvkikRepository + ?Sized,
{
    if !repository.list_items().await?.is_empty() {
        return Ok(false);
    }

    for (value_position, value) in seed_tree().into_iter().enumerate() {
        let value_id = create(
            repository,
            ItemKind::Value,
            None,
            value.title,
            Some(value.description),
            None,
            value_position as i64,
        )
        .await?;

        for (vision_position, vision) in value.visions.into_iter().enumerate() {
            let vision_id = create(
                repository,
                ItemKind::Vision,
                Some(value_id),
                vision.title,
                Some(vision.description),
                None,
                vision_position as i64,
            )
            .await?;

            for (kra_position, kra) in vision.kras.into_iter().enumerate() {
                let kra_id = create(repository, ItemKind::Kra, Some(vision_id), kra.title, None, None, kra_position as i64).await?;

                for (igt_position, igt) in kra.igts.into_iter().enumerate() {
                    let igt_id = create(repository, ItemKind::Igt, Some(kra_id), igt.title, None, None, igt_position as i64).await?;

                    for (kpi_position, kpi) in igt.kpis.into_iter().enumerate() {
                        create(
                            repository,
                            ItemKind::Kpi,
                            Some(igt_id),
                            kpi.title,
                            None,
                            Some((kpi.target_value, kpi.unit, kpi.aggregation)),
                            kpi_position as i64,
                        )
                        .await?;
                    }
                }
            }
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteVvkikRepository;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn repository() -> SqliteVvkikRepository {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool should be created");
        let repository = SqliteVvkikRepository::new(pool);
        repository.init().await.expect("vvkik tables should be created");
        repository
    }

    fn count_by_kind(items: &[domain::VvkikItem], kind: ItemKind) -> usize { items.iter().filter(|item| item.kind == kind).count() }

    #[tokio::test]
    async fn seeds_empty_database_with_two_children_per_level() {
        let repository = repository().await;

        let seeded = seed_if_empty(&repository).await.expect("seeding should succeed");
        assert!(seeded);

        let items = repository.list_items().await.expect("items should be listed");
        assert_eq!(items.len(), 62);
        assert_eq!(count_by_kind(&items, ItemKind::Value), 2);
        assert_eq!(count_by_kind(&items, ItemKind::Vision), 4);
        assert_eq!(count_by_kind(&items, ItemKind::Kra), 8);
        assert_eq!(count_by_kind(&items, ItemKind::Igt), 16);
        assert_eq!(count_by_kind(&items, ItemKind::Kpi), 32);

        // 루트가 아닌 항목은 모두 한 단계 위 종류의 부모를 가리켜야 한다.
        for item in &items {
            match item.kind {
                | ItemKind::Value => assert_eq!(item.parent_id, None),
                | _ => {
                    let parent_id = item.parent_id.expect("non-value items should have a parent");
                    let parent = items.iter().find(|candidate| candidate.id == parent_id).expect("parent should be seeded too");
                    assert!(item.kind.allows_parent(parent.kind));
                },
            }
        }

        // 누적형 KPI에는 합계 집계가 지정되어 있어야 한다.
        let commits = items.iter().find(|item| item.title == "월 커밋 수").expect("seeded kpi should exist");
        assert_eq!(commits.aggregation, KpiAggregation::Sum);
        let body_fat = items.iter().find(|item| item.title == "체지방률").expect("seeded kpi should exist");
        assert_eq!(body_fat.aggregation, KpiAggregation::Latest);
    }

    #[tokio::test]
    async fn seeding_twice_does_not_duplicate_items() {
        let repository = repository().await;

        assert!(seed_if_empty(&repository).await.expect("first seeding should succeed"));
        assert!(!seed_if_empty(&repository).await.expect("second seeding should be skipped"));

        let items = repository.list_items().await.expect("items should be listed");
        assert_eq!(items.len(), 62);
    }

    #[tokio::test]
    async fn does_not_seed_when_user_data_exists() {
        let repository = repository().await;
        repository
            .create_item(VvkikItem::new(NewVvkikItem {
                kind: ItemKind::Value,
                parent_id: None,
                title: "User value".to_string(),
                description: None,
                target_value: None,
                current_value: None,
                unit: None,
                position: 0,
                aggregation: KpiAggregation::default(),
            }))
            .await
            .expect("user item should be created");

        assert!(!seed_if_empty(&repository).await.expect("seeding check should succeed"));

        let items = repository.list_items().await.expect("items should be listed");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "User value");
    }
}
