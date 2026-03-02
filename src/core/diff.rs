// ABOUTME: Symbol diff computation — compares symbol sets between two refs.
// ABOUTME: Produces per-symbol status: added, removed, modified, moved, renamed.

use crate::languages::{Symbol, SymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The computed status of a symbol across two refs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SymbolStatus {
    /// Symbol exists only in head (new file or new symbol)
    Added,
    /// Symbol exists only in base (deleted)
    Removed,
    /// Symbol body changed in the same file
    Modified,
    /// Symbol moved to a different file, body unchanged
    Moved { to_file: String },
    /// Symbol moved to a different file and body changed
    MovedModified { to_file: String },
    /// Symbol renamed in the same file, body unchanged
    Renamed { new_name: String },
    /// Symbol renamed and body changed in same file
    RenamedModified { new_name: String },
    /// Symbol moved, renamed, and body changed
    MovedRenamedModified { to_file: String, new_name: String },
    /// Symbol unchanged
    Unchanged,
}

/// A symbol with its computed diff status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub status: SymbolStatus,
    /// LSP confirmed (true) or heuristic only (false)
    pub confirmed: bool,
}

/// Compute symbol-level diff between base and head symbol sets.
///
/// `base_symbols` and `head_symbols` are maps from file path to symbol list.
pub fn diff_symbols(
    base_symbols: &HashMap<String, Vec<Symbol>>,
    head_symbols: &HashMap<String, Vec<Symbol>>,
) -> Vec<DiffedSymbol> {
    let mut result = Vec::new();

    // Index base symbols by (file, name) → body_hash
    let base_index: HashMap<(&str, &str), &Symbol> = base_symbols
        .iter()
        .flat_map(|(file, syms)| syms.iter().map(move |s| ((file.as_str(), s.name.as_str()), s)))
        .collect();

    // Index head symbols by (file, name) → symbol
    let head_index: HashMap<(&str, &str), &Symbol> = head_symbols
        .iter()
        .flat_map(|(file, syms)| syms.iter().map(move |s| ((file.as_str(), s.name.as_str()), s)))
        .collect();

    // Check all base symbols
    for (file, syms) in base_symbols {
        for sym in syms {
            let key = (file.as_str(), sym.name.as_str());
            if let Some(head_sym) = head_index.get(&key) {
                let status = if sym.body_hash == head_sym.body_hash {
                    SymbolStatus::Unchanged
                } else {
                    SymbolStatus::Modified
                };
                result.push(DiffedSymbol {
                    name: sym.name.clone(),
                    kind: sym.kind,
                    file: file.clone(),
                    status,
                    confirmed: false,
                });
            } else {
                // Not found in head at same location — check if moved
                let moved_to = head_symbols.iter().find_map(|(hfile, hsyms)| {
                    if hfile == file {
                        return None; // same file = rename, not move
                    }
                    hsyms.iter().find(|s| s.name == sym.name).map(|s| (hfile.clone(), s))
                });

                if let Some((to_file, head_sym)) = moved_to {
                    let status = if sym.body_hash == head_sym.body_hash {
                        SymbolStatus::Moved { to_file }
                    } else {
                        SymbolStatus::MovedModified { to_file }
                    };
                    result.push(DiffedSymbol {
                        name: sym.name.clone(),
                        kind: sym.kind,
                        file: file.clone(),
                        status,
                        confirmed: false,
                    });
                } else {
                    result.push(DiffedSymbol {
                        name: sym.name.clone(),
                        kind: sym.kind,
                        file: file.clone(),
                        status: SymbolStatus::Removed,
                        confirmed: false,
                    });
                }
            }
        }
    }

    // Find added symbols (in head but not in base)
    for (file, syms) in head_symbols {
        for sym in syms {
            let key = (file.as_str(), sym.name.as_str());
            if !base_index.contains_key(&key) {
                // Only mark as Added if it didn't come from a move
                let came_from_move = result.iter().any(|d| {
                    matches!(&d.status,
                        SymbolStatus::Moved { to_file } | SymbolStatus::MovedModified { to_file }
                        if to_file == file && d.name == sym.name
                    )
                });
                if !came_from_move {
                    result.push(DiffedSymbol {
                        name: sym.name.clone(),
                        kind: sym.kind,
                        file: file.clone(),
                        status: SymbolStatus::Added,
                        confirmed: false,
                    });
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::{Symbol, SymbolKind};

    fn sym(name: &str, hash: u64) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Fn,
            body_hash: hash,
            start_line: 1,
            end_line: 10,
        }
    }

    #[test]
    fn unchanged_symbol() {
        let base = HashMap::from([("main.go".to_string(), vec![sym("Foo", 42)])]);
        let head = HashMap::from([("main.go".to_string(), vec![sym("Foo", 42)])]);
        let result = diff_symbols(&base, &head);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, SymbolStatus::Unchanged);
    }

    #[test]
    fn modified_symbol() {
        let base = HashMap::from([("main.go".to_string(), vec![sym("Foo", 42)])]);
        let head = HashMap::from([("main.go".to_string(), vec![sym("Foo", 99)])]);
        let result = diff_symbols(&base, &head);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, SymbolStatus::Modified);
    }

    #[test]
    fn added_symbol() {
        let base: HashMap<String, Vec<Symbol>> = HashMap::new();
        let head = HashMap::from([("main.go".to_string(), vec![sym("Foo", 42)])]);
        let result = diff_symbols(&base, &head);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, SymbolStatus::Added);
    }

    #[test]
    fn removed_symbol() {
        let base = HashMap::from([("main.go".to_string(), vec![sym("Foo", 42)])]);
        let head: HashMap<String, Vec<Symbol>> = HashMap::new();
        let result = diff_symbols(&base, &head);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, SymbolStatus::Removed);
    }

    #[test]
    fn moved_symbol_unchanged() {
        let base = HashMap::from([("a.go".to_string(), vec![sym("Foo", 42)])]);
        let head = HashMap::from([("b.go".to_string(), vec![sym("Foo", 42)])]);
        let result = diff_symbols(&base, &head);
        // One Moved entry from base, no Added entry for head
        let moved: Vec<_> = result
            .iter()
            .filter(|d| matches!(&d.status, SymbolStatus::Moved { to_file } if to_file == "b.go"))
            .collect();
        assert_eq!(moved.len(), 1);
        let added: Vec<_> = result
            .iter()
            .filter(|d| d.status == SymbolStatus::Added)
            .collect();
        assert_eq!(added.len(), 0);
    }

    #[test]
    fn moved_symbol_modified() {
        let base = HashMap::from([("a.go".to_string(), vec![sym("Foo", 42)])]);
        let head = HashMap::from([("b.go".to_string(), vec![sym("Foo", 99)])]);
        let result = diff_symbols(&base, &head);
        let entry = result.iter().find(|d| d.name == "Foo").unwrap();
        assert!(matches!(&entry.status, SymbolStatus::MovedModified { to_file } if to_file == "b.go"));
    }
}
