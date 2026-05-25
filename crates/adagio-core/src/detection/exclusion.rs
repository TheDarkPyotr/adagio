use crate::types::RelativePath;

/// Determine whether a path should be excluded from sync.
///
/// Checks against a list of glob patterns (using simple `*` and `?` matching).
/// Returns `true` if the path matches any pattern and should be excluded.
pub fn is_excluded(path: &RelativePath, patterns: &[String]) -> bool {
    let path_str = path.as_str();
    let file_name = path_str.rsplit('/').next().unwrap_or(path_str);
    patterns
        .iter()
        .any(|p| glob_match(p, file_name) || glob_match(p, path_str))
}

/// Minimal glob matching: supports `*` (any chars except `/`) and `?` (one char).
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, &t)
}

fn glob_match_inner(p: &[char], t: &[char]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // '*' matches zero or more non-'/' characters.
            // Try consuming zero chars, one char, ...
            for i in 0..=t.len() {
                if t[..i].contains(&'/') {
                    break; // '*' does not cross path segments
                }
                if glob_match_inner(&p[1..], &t[i..]) {
                    return true;
                }
            }
            false
        }
        (Some('?'), Some(c)) if *c != '/' => glob_match_inner(&p[1..], &t[1..]),
        (Some(pc), Some(tc)) if pc == tc => glob_match_inner(&p[1..], &t[1..]),
        _ => false,
    }
}

/// Filter a list of items, retaining only those NOT matched by any exclusion pattern.
pub fn filter_excluded<T: AsRef<RelativePath>>(
    items: Vec<T>,
    patterns: &[String],
    get_path: impl Fn(&T) -> &RelativePath,
) -> Vec<T> {
    if patterns.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| !is_excluded(get_path(item), patterns))
        .collect()
}

/// Filter a list of items by selective-sync paths.
///
/// If `selective_paths` is empty, all items are retained (sync everything).
/// Otherwise, only items whose path starts with one of the selective paths
/// (or exactly matches) are included.
pub fn filter_selective<T>(items: Vec<T>, selective_paths: &[RelativePath]) -> Vec<T>
where
    T: AsRef<RelativePath>,
{
    if selective_paths.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| {
            let item_path = item.as_ref().as_str();
            selective_paths.iter().any(|sel| {
                let sel_str = sel.as_str();
                // Exact match or path is under the selective directory.
                item_path == sel_str || item_path.starts_with(&format!("{sel_str}/"))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RelativePath;

    #[test]
    fn exact_filename_match() {
        let p = RelativePath::new("docs/.DS_Store");
        assert!(is_excluded(&p, &[".DS_Store".to_string()]));
    }

    #[test]
    fn glob_wildcard_extension() {
        let p = RelativePath::new("work/report.tmp");
        assert!(is_excluded(&p, &["*.tmp".to_string()]));
    }

    #[test]
    fn glob_prefix_wildcard() {
        let p = RelativePath::new("docs/~$budget.xlsx");
        assert!(is_excluded(&p, &["~$*".to_string()]));
    }

    #[test]
    fn no_match_returns_false() {
        let p = RelativePath::new("docs/report.pdf");
        assert!(!is_excluded(
            &p,
            &["*.tmp".to_string(), ".DS_Store".to_string()]
        ));
    }

    #[test]
    fn empty_patterns_never_exclude() {
        let p = RelativePath::new("anything.log");
        assert!(!is_excluded(&p, &[]));
    }

    #[test]
    fn lock_file_pattern() {
        let p = RelativePath::new(".~lock.document.odt#");
        assert!(is_excluded(&p, &[".~lock.*".to_string()]));
    }
}
