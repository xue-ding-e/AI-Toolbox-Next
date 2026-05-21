use super::types::ModelPricing;
use crate::db::SqliteDbState;
use rusqlite::params;
use rust_decimal::Decimal;
use std::str::FromStr;

pub fn get_model_pricing_list(db_state: &SqliteDbState) -> Result<Vec<ModelPricing>, String> {
    db_state.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT model_id, display_name, input_cost_per_million, output_cost_per_million,
                        cache_read_cost_per_million, cache_creation_cost_per_million
                 FROM model_pricing
                 ORDER BY LOWER(display_name), LOWER(model_id)",
            )
            .map_err(|error| format!("Failed to prepare model pricing query: {error}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ModelPricing {
                    model_id: row.get(0)?,
                    display_name: row.get(1)?,
                    input_cost_per_million: row.get(2)?,
                    output_cost_per_million: row.get(3)?,
                    cache_read_cost_per_million: row.get(4)?,
                    cache_creation_cost_per_million: row.get(5)?,
                })
            })
            .map_err(|error| format!("Failed to query model pricing list: {error}"))?;

        let mut pricing_list = Vec::new();
        for row in rows {
            pricing_list
                .push(row.map_err(|error| format!("Failed to read model pricing row: {error}"))?);
        }
        Ok(pricing_list)
    })
}

pub fn upsert_model_pricing(
    db_state: &SqliteDbState,
    pricing: ModelPricing,
) -> Result<ModelPricing, String> {
    let normalized_pricing = normalize_model_pricing(pricing)?;
    db_state.with_conn(|conn| {
        conn.execute(
            "INSERT INTO model_pricing (
                model_id, display_name, input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_creation_cost_per_million
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(model_id) DO UPDATE SET
                display_name = excluded.display_name,
                input_cost_per_million = excluded.input_cost_per_million,
                output_cost_per_million = excluded.output_cost_per_million,
                cache_read_cost_per_million = excluded.cache_read_cost_per_million,
                cache_creation_cost_per_million = excluded.cache_creation_cost_per_million",
            params![
                normalized_pricing.model_id,
                normalized_pricing.display_name,
                normalized_pricing.input_cost_per_million,
                normalized_pricing.output_cost_per_million,
                normalized_pricing.cache_read_cost_per_million,
                normalized_pricing.cache_creation_cost_per_million,
            ],
        )
        .map_err(|error| format!("Failed to upsert model pricing: {error}"))?;
        Ok(())
    })?;
    Ok(normalized_pricing)
}

pub fn delete_model_pricing(db_state: &SqliteDbState, model_id: String) -> Result<(), String> {
    let model_id = model_id.trim().to_string();
    if model_id.is_empty() {
        return Err("Model ID is required".to_string());
    }

    db_state.with_conn(|conn| {
        conn.execute(
            "DELETE FROM model_pricing WHERE model_id = ?1",
            params![model_id],
        )
        .map_err(|error| format!("Failed to delete model pricing: {error}"))?;
        Ok(())
    })
}

fn normalize_model_pricing(pricing: ModelPricing) -> Result<ModelPricing, String> {
    let model_id = pricing.model_id.trim().to_string();
    if model_id.is_empty() {
        return Err("Model ID is required".to_string());
    }

    let display_name = pricing.display_name.trim().to_string();
    if display_name.is_empty() {
        return Err("Display name is required".to_string());
    }

    Ok(ModelPricing {
        model_id,
        display_name,
        input_cost_per_million: validate_non_negative_decimal(
            "input_cost_per_million",
            &pricing.input_cost_per_million,
        )?,
        output_cost_per_million: validate_non_negative_decimal(
            "output_cost_per_million",
            &pricing.output_cost_per_million,
        )?,
        cache_read_cost_per_million: validate_non_negative_decimal(
            "cache_read_cost_per_million",
            &pricing.cache_read_cost_per_million,
        )?,
        cache_creation_cost_per_million: validate_non_negative_decimal(
            "cache_creation_cost_per_million",
            &pricing.cache_creation_cost_per_million,
        )?,
    })
}

fn validate_non_negative_decimal(label: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required"));
    }
    let parsed = Decimal::from_str(trimmed)
        .map_err(|error| format!("{label} must be a non-negative decimal: {error}"))?;
    if parsed < Decimal::ZERO {
        return Err(format!("{label} must be non-negative"));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pricing() -> ModelPricing {
        ModelPricing {
            model_id: "test-model-pricing-crud".to_string(),
            display_name: "Test Model Pricing".to_string(),
            input_cost_per_million: "1.25".to_string(),
            output_cost_per_million: "2.5".to_string(),
            cache_read_cost_per_million: "0.125".to_string(),
            cache_creation_cost_per_million: "0.75".to_string(),
        }
    }

    #[test]
    fn model_pricing_upsert_list_and_delete_round_trip() {
        let db_state = SqliteDbState::in_memory_for_test().expect("sqlite");

        let saved = upsert_model_pricing(&db_state, sample_pricing()).expect("upsert");
        assert_eq!(saved.model_id, "test-model-pricing-crud");

        let list = get_model_pricing_list(&db_state).expect("list");
        assert!(list
            .iter()
            .any(|pricing| pricing.model_id == "test-model-pricing-crud"
                && pricing.input_cost_per_million == "1.25"));

        delete_model_pricing(&db_state, "test-model-pricing-crud".to_string()).expect("delete");
        let list = get_model_pricing_list(&db_state).expect("list after delete");
        assert!(!list
            .iter()
            .any(|pricing| pricing.model_id == "test-model-pricing-crud"));
    }

    #[test]
    fn model_pricing_rejects_negative_cost() {
        let db_state = SqliteDbState::in_memory_for_test().expect("sqlite");
        let mut pricing = sample_pricing();
        pricing.input_cost_per_million = "-1".to_string();

        assert!(upsert_model_pricing(&db_state, pricing).is_err());
    }
}
