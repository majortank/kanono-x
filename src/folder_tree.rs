use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FolderTreeNode {
    pub name: String,
    pub full_path: PathBuf,
    pub direct_tracks: usize,
    pub total_tracks: usize,
    pub children: BTreeMap<String, FolderTreeNode>,
}

impl FolderTreeNode {
    pub fn new(name: String, full_path: PathBuf) -> Self {
        Self {
            name,
            full_path,
            direct_tracks: 0,
            total_tracks: 0,
            children: BTreeMap::new(),
        }
    }

    pub fn insert_relative(&mut self, rel_components: &[String], target_full_path: &Path) {
        self.total_tracks += 1;
        if rel_components.is_empty() {
            self.direct_tracks += 1;
            return;
        }

        let first = &rel_components[0];
        let child_full_path = self.full_path.join(first);

        let child = self.children.entry(first.clone()).or_insert_with(|| {
            FolderTreeNode::new(first.clone(), child_full_path)
        });

        child.insert_relative(&rel_components[1..], target_full_path);
    }
}

#[derive(Debug, Clone, Default)]
pub struct FolderHierarchy {
    pub roots: BTreeMap<String, FolderTreeNode>,
    pub expanded_paths: HashSet<PathBuf>,
    pub selected_path: Option<PathBuf>,
    pub include_subfolders: bool,
}

impl FolderHierarchy {
    pub fn new() -> Self {
        Self {
            roots: BTreeMap::new(),
            expanded_paths: HashSet::new(),
            selected_path: None,
            include_subfolders: true,
        }
    }

    /// Build folder tree from all track paths
    pub fn rebuild_from_paths<'a, I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        self.roots.clear();

        for p_str in paths {
            let path = Path::new(p_str);
            let Some(parent) = path.parent() else {
                continue;
            };

            // Normalize path string
            let parent_buf = parent.to_path_buf();
            let parent_str = parent_buf.to_string_lossy();

            // Identify root directory
            let (root_label, root_dir, rel_path) = if parent_str.starts_with("/home/") {
                // E.g. /home/user/Music/Jazz -> root is /home/user/Music
                let parts: Vec<&str> = parent_str.split('/').filter(|s| !s.is_empty()).collect();
                if parts.len() >= 3 {
                    // /home/username/Music or top folder
                    let base_parts = &parts[..parts.len().min(3)];
                    let root_path_str = format!("/{}", base_parts.join("/"));
                    let root_buf = PathBuf::from(&root_path_str);
                    let label = base_parts.last().copied().unwrap_or("Music").to_string();
                    let rel = parent.strip_prefix(&root_buf).unwrap_or(Path::new("")).to_path_buf();
                    (label, root_buf, rel)
                } else {
                    (parent_str.to_string(), parent_buf.clone(), PathBuf::new())
                }
            } else if parent_str.starts_with('/') {
                let parts: Vec<&str> = parent_str.split('/').filter(|s| !s.is_empty()).collect();
                let label = parts.first().copied().unwrap_or("Root").to_string();
                let root_buf = PathBuf::from(format!("/{}", label));
                let rel = parent.strip_prefix(&root_buf).unwrap_or(Path::new("")).to_path_buf();
                (label, root_buf, rel)
            } else {
                // Relative path like "demo_tracks" or "library"
                let first = parent_str.split('/').next().unwrap_or("library").to_string();
                let root_buf = PathBuf::from(&first);
                let rel = parent.strip_prefix(&root_buf).unwrap_or(Path::new("")).to_path_buf();
                (first.clone(), root_buf, rel)
            };

            let root_node = self.roots.entry(root_label).or_insert_with(|| {
                FolderTreeNode::new(root_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Root".to_string()), root_dir.clone())
            });

            let rel_components: Vec<String> = rel_path
                .components()
                .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty() && s != ".")
                .collect();

            root_node.insert_relative(&rel_components, &parent_buf);
        }

        // Auto-expand roots by default
        for root in self.roots.values() {
            self.expanded_paths.insert(root.full_path.clone());
        }
    }

    pub fn toggle_expanded(&mut self, path: &Path) {
        if self.expanded_paths.contains(path) {
            self.expanded_paths.remove(path);
        } else {
            self.expanded_paths.insert(path.to_path_buf());
        }
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_paths.contains(path)
    }

    pub fn expand_all(&mut self) {
        fn collect_all(node: &FolderTreeNode, set: &mut HashSet<PathBuf>) {
            set.insert(node.full_path.clone());
            for child in node.children.values() {
                collect_all(child, set);
            }
        }
        for root in self.roots.values() {
            collect_all(root, &mut self.expanded_paths);
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded_paths.clear();
    }

    pub fn matches_track(&self, track_file_path: Option<&str>) -> bool {
        let Some(selected) = &self.selected_path else {
            return true; // No folder filter selected
        };

        let Some(path_str) = track_file_path else {
            return false;
        };

        let track_path = Path::new(path_str);
        let Some(parent) = track_path.parent() else {
            return false;
        };

        if self.include_subfolders {
            parent.starts_with(selected)
        } else {
            parent == selected
        }
    }
}
