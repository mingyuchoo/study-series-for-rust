//! Lightweight COBOL structure analyzer (paragraphs, PERFORM/CALL graph, EXEC SQL lineage, COPY).
//! Deterministic tooling that complements the LLM (design §3).
use super::AnalyzedFile;
use amap_domain::*;
use regex::Regex;
use std::sync::OnceLock;

struct Res {
    program: Regex,
    paragraph: Regex,
    perform: Regex,
    call: Regex,
    copy: Regex,
    sql_read: Regex,
    sql_write: Regex,
    division: Regex,
}

fn res() -> &'static Res {
    static R: OnceLock<Res> = OnceLock::new();
    R.get_or_init(|| Res {
        program: Regex::new(r"(?i)PROGRAM-ID\.\s+([A-Z0-9-]+)").unwrap(),
        paragraph: Regex::new(r"(?m)^\s{0,7}([A-Z0-9][A-Z0-9-]*)\.\s*$").unwrap(),
        perform: Regex::new(r"(?i)\bPERFORM\s+([A-Z0-9][A-Z0-9-]*)").unwrap(),
        call: Regex::new(r#"(?i)\bCALL\s+['"]([A-Z0-9-]+)['"]"#).unwrap(),
        copy: Regex::new(r"(?i)\bCOPY\s+([A-Z0-9-]+)").unwrap(),
        sql_read: Regex::new(r"(?is)EXEC\s+SQL\s+SELECT.*?\bFROM\s+([A-Z0-9_]+)").unwrap(),
        sql_write: Regex::new(r"(?is)EXEC\s+SQL\s+(?:UPDATE\s+([A-Z0-9_]+)|INSERT\s+INTO\s+([A-Z0-9_]+)|DELETE\s+FROM\s+([A-Z0-9_]+))").unwrap(),
        division: Regex::new(r"(?i)^\s*(IDENTIFICATION|ENVIRONMENT|DATA|PROCEDURE)\s+DIVISION").unwrap(),
    })
}

pub fn analyze(file: &str, text: &str) -> AnalyzedFile {
    let r = res();
    let program = r.program.captures(text).map(|c| c[1].to_string()).unwrap_or_else(|| file.to_string());
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len() as u32;

    // Find the PROCEDURE DIVISION start so paragraph detection ignores data definitions.
    let proc_start = lines.iter().position(|l| r.division.is_match(l) && l.to_uppercase().contains("PROCEDURE")).unwrap_or(0);

    let mut paragraphs: Vec<(String, u32)> = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(proc_start) {
        let stripped = if line.len() > 6 { &line[6..] } else { line };
        if let Some(c) = r.paragraph.captures(stripped) {
            let name = c[1].to_string();
            if name.contains("DIVISION") || name.contains("SECTION") {
                continue;
            }
            paragraphs.push((name, i as u32 + 1));
        }
    }

    let mut entities = Vec::new();
    let mut relationships = Vec::new();
    let mut db_entities = Vec::new();
    let mut interfaces = Vec::new();
    let prog_id = SourceUnitId::new(format!("{file}::{program}"));
    let mut program_entity = CodeEntity { id: prog_id.clone(), language: Language::Cobol, kind: EntityKind::Program, symbol: program.clone(), location: SourceLocation::new(file, 1, total), dependencies: vec![], function_id: None, suspicious: false };

    for (idx, (name, start)) in paragraphs.iter().enumerate() {
        let end = paragraphs.get(idx + 1).map(|(_, s)| s - 1).unwrap_or(total);
        let body: String = lines[(*start as usize - 1)..(end as usize).min(lines.len())].join("\n");
        let id = SourceUnitId::new(format!("{file}::{name}"));
        let mut deps = Vec::new();
        for c in r.perform.captures_iter(&body) {
            let target = c[1].to_string();
            if paragraphs.iter().any(|(p, _)| *p == target) {
                let tid = SourceUnitId::new(format!("{file}::{target}"));
                relationships.push(Relationship::new(id.0.clone(), RelationKind::SourceCalls, tid.0.clone()));
                deps.push(tid);
            }
        }
        for c in r.call.captures_iter(&body) {
            let target = c[1].to_string();
            let iid = InterfaceId::new(format!("IF-CALL-{target}"));
            interfaces.push(InterfaceSpec { id: iid.clone(), name: target.clone(), kind: "call".into(), direction: "out".into() });
            deps.push(SourceUnitId::new(format!("external::{target}")));
        }
        for c in r.sql_read.captures_iter(&body) {
            let table = c[1].to_uppercase();
            let did = DbEntityId::new(table.clone());
            db_entities.push(DbEntity { id: did.clone(), table: table.clone(), column: None, data_type: None });
            relationships.push(Relationship::new(id.0.clone(), RelationKind::SourceReadsDb, did.0));
        }
        for c in r.sql_write.captures_iter(&body) {
            let table = c.get(1).or(c.get(2)).or(c.get(3)).map(|m| m.as_str().to_uppercase()).unwrap_or_default();
            let did = DbEntityId::new(table.clone());
            db_entities.push(DbEntity { id: did.clone(), table, column: None, data_type: None });
            relationships.push(Relationship::new(id.0.clone(), RelationKind::SourceWritesDb, did.0));
        }
        entities.push(CodeEntity { id: id.clone(), language: Language::Cobol, kind: EntityKind::Paragraph, symbol: name.clone(), location: SourceLocation::new(file, *start, end), dependencies: deps, function_id: None, suspicious: false });
        if idx == 0 {
            program_entity.dependencies.push(id);
        }
    }
    for c in r.copy.captures_iter(text) {
        let cb = c[1].to_string();
        entities.push(CodeEntity { id: SourceUnitId::new(format!("copybook::{cb}")), language: Language::Cobol, kind: EntityKind::Copybook, symbol: cb.clone(), location: SourceLocation::new(file, 1, 1), dependencies: vec![], function_id: None, suspicious: false });
        program_entity.dependencies.push(SourceUnitId::new(format!("copybook::{cb}")));
    }
    entities.insert(0, program_entity);
    db_entities.sort_by(|a, b| a.id.cmp(&b.id));
    db_entities.dedup_by(|a, b| a.id == b.id);
    AnalyzedFile { entities, db_entities, relationships, interfaces }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_paragraphs_and_lineage() {
        let src = r#"       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOAN231.
       PROCEDURE DIVISION.
       0000-MAIN.
           PERFORM 1000-READ-CUSTOMER
           PERFORM 2000-CALC-FEE
           STOP RUN.
       1000-READ-CUSTOMER.
           EXEC SQL SELECT GRADE INTO :WS-GRADE FROM CUSTOMER WHERE ID = :WS-ID END-EXEC.
       2000-CALC-FEE.
           EXEC SQL UPDATE LOAN_MASTER SET FEE = :WS-FEE END-EXEC.
           CALL 'IF238' USING WS-REC.
       9000-UNUSED.
           DISPLAY 'never'.
"#;
        let a = analyze("LOAN231.cbl", src);
        let names: Vec<_> = a.entities.iter().map(|e| e.symbol.as_str()).collect();
        assert!(names.contains(&"2000-CALC-FEE") && names.contains(&"LOAN231"));
        assert!(a.db_entities.iter().any(|d| d.table == "CUSTOMER"));
        assert!(a.relationships.iter().any(|r| r.kind == RelationKind::SourceWritesDb && r.to == "LOAN_MASTER"));
        assert!(a.interfaces.iter().any(|i| i.name == "IF238"));
    }
}
