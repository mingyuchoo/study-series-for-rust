//! Deterministic SQL and JCL structure discovery.
use super::AnalyzedFile;
use amap_domain::*;
use regex::Regex;

fn empty() -> AnalyzedFile {
    AnalyzedFile {
        entities: vec![],
        db_entities: vec![],
        relationships: vec![],
        interfaces: vec![],
    }
}

pub fn analyze_sql(file: &str, text: &str) -> AnalyzedFile {
    let mut out = empty();
    let declaration = Regex::new(
        r"(?i)\bCREATE\s+(?:OR\s+REPLACE\s+)?(PROCEDURE|FUNCTION|VIEW|TABLE)\s+([A-Z_][A-Z0-9_.$]*)",
    )
    .unwrap();
    let reads = Regex::new(r"(?i)\b(?:FROM|JOIN)\s+([A-Z_][A-Z0-9_.$]*)").unwrap();
    let writes = Regex::new(
        r"(?i)\b(?:UPDATE|INSERT\s+INTO|DELETE\s+FROM|MERGE\s+INTO)\s+([A-Z_][A-Z0-9_.$]*)",
    )
    .unwrap();
    let total = text.lines().count().max(1) as u32;
    let root_id = SourceUnitId::new(format!("{file}::sql"));
    out.entities.push(CodeEntity {
        id: root_id.clone(),
        language: Language::Sql,
        kind: EntityKind::Procedure,
        symbol: file.to_string(),
        location: SourceLocation::new(file, 1, total),
        dependencies: vec![],
        function_id: None,
        suspicious: false,
    });
    for capture in declaration.captures_iter(text) {
        let symbol = capture[2].to_uppercase();
        out.entities.push(CodeEntity {
            id: SourceUnitId::new(format!("{file}::{symbol}")),
            language: Language::Sql,
            kind: if capture[1].eq_ignore_ascii_case("TABLE") {
                EntityKind::Table
            } else {
                EntityKind::Procedure
            },
            symbol,
            location: SourceLocation::new(file, 1, total),
            dependencies: vec![],
            function_id: None,
            suspicious: false,
        });
    }
    for (regex, relation) in [
        (&reads, RelationKind::SourceReadsDb),
        (&writes, RelationKind::SourceWritesDb),
    ] {
        for capture in regex.captures_iter(text) {
            let table = capture[1].to_uppercase();
            let id = DbEntityId::new(table.clone());
            if !out.db_entities.iter().any(|entity| entity.id == id) {
                out.db_entities.push(DbEntity {
                    id: id.clone(),
                    table,
                    column: None,
                    data_type: None,
                });
            }
            out.relationships
                .push(Relationship::new(root_id.0.clone(), relation, id.0));
        }
    }
    out
}

pub fn analyze_jcl(file: &str, text: &str) -> AnalyzedFile {
    let mut out = empty();
    let step = Regex::new(r"(?im)^//([A-Z0-9@$#]+)\s+EXEC\s+(?:PGM=)?([A-Z0-9@$#.-]+)").unwrap();
    let dataset = Regex::new(r"(?im)\bDSN=([A-Z0-9@$#.-]+)").unwrap();
    let total = text.lines().count().max(1) as u32;
    for capture in step.captures_iter(text) {
        let symbol = capture[1].to_string();
        let program = capture[2].to_string();
        let id = SourceUnitId::new(format!("{file}::{symbol}"));
        let dependency = SourceUnitId::new(format!("external::{program}"));
        out.entities.push(CodeEntity {
            id: id.clone(),
            language: Language::Jcl,
            kind: EntityKind::Job,
            symbol,
            location: SourceLocation::new(file, 1, total),
            dependencies: vec![dependency.clone()],
            function_id: None,
            suspicious: false,
        });
        out.relationships.push(Relationship::new(
            id.0,
            RelationKind::SourceCalls,
            dependency.0,
        ));
        out.interfaces.push(InterfaceSpec {
            id: InterfaceId::new(format!("IF-JCL-{program}")),
            name: program,
            kind: "batch-program".into(),
            direction: "out".into(),
        });
    }
    for capture in dataset.captures_iter(text) {
        let name = capture[1].to_string();
        out.interfaces.push(InterfaceSpec {
            id: InterfaceId::new(format!("IF-DSN-{name}")),
            name,
            kind: "dataset".into(),
            direction: "inout".into(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_sql_lineage() {
        let analyzed = analyze_sql(
            "billing.sql",
            "CREATE VIEW BILL AS SELECT * FROM INVOICE JOIN CUSTOMER ON 1=1; UPDATE LEDGER SET X=1;",
        );
        assert!(analyzed
            .db_entities
            .iter()
            .any(|entity| entity.table == "INVOICE"));
        assert!(analyzed
            .relationships
            .iter()
            .any(|edge| { edge.kind == RelationKind::SourceWritesDb && edge.to == "LEDGER" }));
    }

    #[test]
    fn discovers_jcl_steps() {
        let analyzed = analyze_jcl(
            "nightly.jcl",
            "//PAYSTEP EXEC PGM=PAY001\n//OUT DD DSN=BANK.PAY.OUT",
        );
        assert!(analyzed
            .entities
            .iter()
            .any(|entity| entity.symbol == "PAYSTEP"));
        assert!(analyzed
            .interfaces
            .iter()
            .any(|interface| interface.name == "PAY001"));
    }
}
