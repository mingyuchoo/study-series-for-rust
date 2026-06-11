use super::{SqliteIvkikRepository,
            mapper::row_to_measurement};
use async_trait::async_trait;
use domain::{DomainError,
             KpiMeasurement,
             KpiMeasurementRepository};
use uuid::Uuid;

#[async_trait]
impl KpiMeasurementRepository for SqliteIvkikRepository {
    async fn record_kpi_measurement(&self, measurement: KpiMeasurement) -> Result<KpiMeasurement, DomainError> {
        sqlx::query(
            r#"
            INSERT INTO kpi_measurements (id, kpi_id, value, measured_at, note)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(measurement.id.to_string())
        .bind(measurement.kpi_id.to_string())
        .bind(measurement.value)
        .bind(measurement.measured_at.to_rfc3339())
        .bind(&measurement.note)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(measurement)
    }

    async fn list_kpi_measurements(&self, kpi_id: Uuid) -> Result<Vec<KpiMeasurement>, DomainError> {
        let rows = sqlx::query(
            r#"
            SELECT id, kpi_id, value, measured_at, note
            FROM kpi_measurements
            WHERE kpi_id = ?
            ORDER BY measured_at DESC
            "#,
        )
        .bind(kpi_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_measurement).collect()
    }

    async fn list_all_kpi_measurements(&self) -> Result<Vec<KpiMeasurement>, DomainError> {
        let rows = sqlx::query(
            r#"
            SELECT id, kpi_id, value, measured_at, note
            FROM kpi_measurements
            ORDER BY measured_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        rows.iter().map(row_to_measurement).collect()
    }

    async fn delete_kpi_measurement(&self, kpi_id: Uuid, measurement_id: Uuid) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM kpi_measurements WHERE id = ? AND kpi_id = ?")
            .bind(measurement_id.to_string())
            .bind(kpi_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
