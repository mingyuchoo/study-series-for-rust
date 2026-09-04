//! tree-sitter based analysis for Java / Python: classes, methods/functions and call edges.
use super::AnalyzedFile;
use amap_domain::*;
use tree_sitter::{Node, Parser};

pub fn analyze(file: &str, text: &str, lang: Language) -> Option<AnalyzedFile> {
    let mut parser = Parser::new();
    let language = match lang {
        Language::Java => tree_sitter_java::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Javascript => tree_sitter_javascript::LANGUAGE.into(),
        Language::Csharp => tree_sitter_c_sharp::LANGUAGE.into(),
        _ => return None,
    };
    parser.set_language(&language).ok()?;
    let tree = parser.parse(text, None)?;
    let mut entities = Vec::new();
    let mut relationships = Vec::new();
    walk(tree.root_node(), text, file, lang, None, &mut entities);
    // Resolve call targets to entities in the same file and retain external calls explicitly.
    let symbols: Vec<(String, SourceUnitId)> = entities
        .iter()
        .map(|e| {
            (
                e.symbol.rsplit('.').next().unwrap_or(&e.symbol).to_string(),
                e.id.clone(),
            )
        })
        .collect();
    for e in entities.iter_mut() {
        let mut resolved: Vec<SourceUnitId> = e
            .dependencies
            .iter()
            .map(|dependency| {
                let callee = dependency.0.strip_prefix("call::").unwrap_or(&dependency.0);
                symbols
                    .iter()
                    .find(|(symbol, _)| callee == symbol)
                    .map(|(_, id)| id.clone())
                    .unwrap_or_else(|| SourceUnitId::new(format!("external::{callee}")))
            })
            .collect();
        resolved.sort_by(|left, right| left.0.cmp(&right.0));
        resolved.dedup();
        for r in &resolved {
            relationships.push(Relationship::new(
                e.id.0.clone(),
                RelationKind::SourceCalls,
                r.0.clone(),
            ));
        }
        e.dependencies = resolved;
    }
    Some(AnalyzedFile {
        entities,
        db_entities: vec![],
        relationships,
        interfaces: vec![],
    })
}

fn walk(
    node: Node,
    text: &str,
    file: &str,
    lang: Language,
    scope: Option<&str>,
    entities: &mut Vec<CodeEntity>,
) {
    let kind = node.kind();
    let is_class = matches!(
        kind,
        "class_declaration"
            | "class_definition"
            | "interface_declaration"
            | "struct_item"
            | "trait_item"
    );
    let is_fn = matches!(
        kind,
        "method_declaration"
            | "function_definition"
            | "constructor_declaration"
            | "function_declaration"
            | "method_definition"
            | "function_item"
            | "local_function_statement"
    );
    if is_class || is_fn {
        let name = node
            .child_by_field_name("name")
            .map(|n| n.utf8_text(text.as_bytes()).unwrap_or("?").to_string())
            .unwrap_or_else(|| "anonymous".into());
        let symbol = match scope {
            Some(s) => format!("{s}.{name}"),
            None => name.clone(),
        };
        let id = SourceUnitId::new(format!("{file}::{symbol}"));
        let mut deps = Vec::new();
        if is_fn {
            collect_calls(node, text, &mut deps);
        }
        entities.push(CodeEntity {
            id,
            language: lang,
            kind: if is_class {
                EntityKind::Class
            } else if matches!(lang, Language::Java | Language::Csharp) {
                EntityKind::Method
            } else {
                EntityKind::Function
            },
            symbol: symbol.clone(),
            location: SourceLocation::new(
                file,
                node.start_position().row as u32 + 1,
                node.end_position().row as u32 + 1,
            ),
            dependencies: deps,
            function_id: None,
            suspicious: false,
        });
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk(child, text, file, lang, Some(&symbol), entities);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, text, file, lang, scope, entities);
    }
}

fn collect_calls(node: Node, text: &str, out: &mut Vec<SourceUnitId>) {
    if matches!(
        node.kind(),
        "method_invocation" | "call" | "call_expression" | "invocation_expression"
    ) {
        let callee = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("function"))
            .map(|n| n.utf8_text(text.as_bytes()).unwrap_or("").to_string())
            .unwrap_or_default();
        let short = callee.rsplit('.').next().unwrap_or(&callee).to_string();
        if !short.is_empty() {
            out.push(SourceUnitId::new(format!("call::{short}")));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, text, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_methods_and_calls() {
        let src = "class InterestCalculator { double daily(double r) { return r / 365; } double total(double r, int d) { return daily(r) * d; } }";
        let a = analyze("InterestCalculator.java", src, Language::Java).unwrap();
        let total = a
            .entities
            .iter()
            .find(|e| e.symbol.ends_with("total"))
            .unwrap();
        assert!(total.dependencies.iter().any(|d| d.0.ends_with("daily")));
    }
    #[test]
    fn python_functions() {
        let src = "def fee(x):\n    return x\n\ndef post(x):\n    return fee(x)\n";
        let a = analyze("svc.py", src, Language::Python).unwrap();
        assert_eq!(a.entities.len(), 2);
        assert!(a
            .relationships
            .iter()
            .any(|r| r.kind == RelationKind::SourceCalls));
    }
}
