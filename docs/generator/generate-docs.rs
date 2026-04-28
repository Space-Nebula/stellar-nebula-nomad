use std::fs;
use std::path::Path;

fn main() {
    let src_dir = "src";
    let output_file = "docs/CONTRACT_API.md";

    let mut markdown = String::from("# Contract API Documentation\n\n");

    if let Ok(entries) = fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                markdown.push_str(&format!("## Module: {}\n\n", path.file_stem().unwrap().to_str().unwrap()));
                
                let content = fs::read_to_string(&path).unwrap_or_default();
                let mut current_docs = Vec::new();

                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("///") {
                        current_docs.push(trimmed.trim_start_matches("///").trim().to_string());
                    } else if trimmed.starts_with("pub fn") || trimmed.starts_with("pub struct") {
                        if !current_docs.is_empty() {
                            let name = trimmed
                                .split_whitespace()
                                .nth(2)
                                .unwrap_or("Unknown")
                                .split('(')
                                .next()
                                .unwrap_or("Unknown")
                                .trim_end_matches('{');
                            
                            markdown.push_str(&format!("### {}\n", name));
                            for doc in &current_docs {
                                markdown.push_str(&format!("{}\n", doc));
                            }
                            markdown.push_str("\n");
                            current_docs.clear();
                        }
                    } else {
                        current_docs.clear();
                    }
                }
            }
        }
    }

    fs::create_dir_all("docs").unwrap();
    fs::write(output_file, markdown).expect("Unable to write docs");
    println!("Documentation generated to {}", output_file);
}
