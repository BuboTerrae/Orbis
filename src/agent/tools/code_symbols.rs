use crate::agent::workspace::{read_text_file, resolve_path};
use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use serde_json::json;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

fn language_for_extension(ext: &str) -> Option<Language> {
    match ext {
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "ts" | "tsx" | "js" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        _ => None,
    }
}

fn symbol_query_for_language(lang: &Language) -> Option<&'static str> {
    let rust_lang = tree_sitter_rust::LANGUAGE.into();
    let ts_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let py_lang = tree_sitter_python::LANGUAGE.into();
    let go_lang = tree_sitter_go::LANGUAGE.into();

    if *lang == rust_lang {
        Some(
            r#"
            [
                (function_item name: (identifier) @name)
                (struct_item name: (type_identifier) @name)
                (enum_item name: (type_identifier) @name)
                (trait_item name: (type_identifier) @name)
                (impl_item)
                (mod_item name: (identifier) @name)
                (const_item name: (identifier) @name)
                (static_item name: (identifier) @name)
                (type_alias name: (type_identifier) @name)
                (macro_definition name: (identifier) @name)
            ] @symbol
        "#,
        )
    } else if *lang == ts_lang {
        Some(
            r#"
            [
                (function_declaration name: (identifier) @name)
                (method_definition name: (property_identifier) @name)
                (class_declaration name: (type_identifier) @name)
                (interface_declaration name: (type_identifier) @name)
                (type_alias_declaration name: (type_identifier) @name)
                (enum_declaration name: (identifier) @name)
                (export_statement declaration: (_) @symbol)
            ] @symbol
        "#,
        )
    } else if *lang == py_lang {
        Some(
            r#"
            [
                (function_definition name: (identifier) @name)
                (class_definition name: (identifier) @name)
                (async_function_definition name: (identifier) @name)
            ] @symbol
        "#,
        )
    } else if *lang == go_lang {
        Some(
            r#"
            [
                (function_declaration name: (identifier) @name)
                (method_declaration name: (field_identifier) @name)
                (type_declaration (type_spec name: (type_identifier) @name))
                (const_declaration (const_spec name: (identifier) @name))
            ] @symbol
        "#,
        )
    } else {
        None
    }
}

struct Symbol {
    name: String,
    kind: String,
    line: usize,
    #[allow(dead_code)]
    byte_range: std::ops::Range<usize>,
}

fn parse_symbols_with_tree_sitter(content: &str, ext: &str) -> Option<Vec<Symbol>> {
    let lang = language_for_extension(ext)?;
    let query_str = symbol_query_for_language(&lang)?;

    let mut parser = Parser::new();
    parser.set_language(&lang).ok()?;
    let tree = parser.parse(content, None)?;

    let query = Query::new(&lang, query_str).ok()?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut symbols = Vec::new();

    while let Some(match_) = matches.next() {
        let mut name = String::new();
        let mut kind = String::new();
        let mut start_byte = 0;
        let mut end_byte = 0;
        let mut start_line = 0;

        for capture in match_.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            let node = capture.node;
            let text = &content[node.byte_range()];

            match capture_name {
                "name" => name = text.to_string(),
                "symbol" => {
                    kind = node.kind().to_string();
                    start_byte = node.start_byte();
                    end_byte = node.end_byte();
                    start_line = node.start_position().row + 1;
                }
                _ => {}
            }
        }

        if !name.is_empty() || !kind.is_empty() {
            symbols.push(Symbol {
                name: if name.is_empty() { kind.clone() } else { name },
                kind,
                line: start_line,
                byte_range: start_byte..end_byte,
            });
        }
    }

    Some(symbols)
}

fn fallback_regex_parse(content: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let kind = if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
        {
            "function"
        } else if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
            "struct"
        } else if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") {
            "enum"
        } else if trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ") {
            "trait"
        } else if trimmed.starts_with("impl ") {
            "impl"
        } else if trimmed.starts_with("pub mod ") || trimmed.starts_with("mod ") {
            "mod"
        } else if trimmed.starts_with("class ") {
            "class"
        } else if trimmed.starts_with("def ") {
            "function"
        } else if trimmed.starts_with("interface ") {
            "interface"
        } else if trimmed.starts_with("type ") {
            "type"
        } else {
            continue;
        };
        symbols.push(Symbol {
            name: trimmed
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .trim_end_matches('(')
                .trim_end_matches('{')
                .to_string(),
            kind: kind.to_string(),
            line: idx + 1,
            byte_range: 0..0,
        });
    }
    symbols
}

pub fn find_symbols(args: &serde_json::Value) -> Result<String> {
    let path_str = args["path"]
        .as_str()
        .context("Missing 'path' argument in find_symbols")?;

    let path = resolve_path(path_str);
    if !path.exists() {
        anyhow::bail!("Path '{}' does not exist", path.display());
    }

    let content = read_text_file(&path)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let symbols = if let Some(tree_sitter_symbols) = parse_symbols_with_tree_sitter(&content, ext) {
        if !tree_sitter_symbols.is_empty() {
            tree_sitter_symbols
        } else {
            fallback_regex_parse(&content)
        }
    } else {
        fallback_regex_parse(&content)
    };

    if symbols.is_empty() {
        Ok(format!("No symbols found in '{}'", path.display()))
    } else {
        let output: Vec<String> = symbols
            .into_iter()
            .map(|s| format!("{:4} {:12} {}", s.line, s.kind, s.name))
            .collect();
        Ok(format!(
            "Symbols in '{}':\n{}",
            path.display(),
            output.join("\n")
        ))
    }
}

pub fn get_symbol_tools_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "find_symbols".to_string(),
        description: "List symbol declarations (functions, structs, impls, classes, traits, etc.) in a source code file using tree-sitter for accurate parsing.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Source code file path" }
            },
            "required": ["path"]
        }),
    }]
}
