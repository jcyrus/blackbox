use anyhow::Result;
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

pub struct FileTree {
    root: PathBuf,
    pub nodes: Vec<FileNode>,
    pub selected: usize,
    expanded: HashSet<PathBuf>,
    pub create_input: String,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Result<Self> {
        let mut expanded = HashSet::new();
        expanded.insert(root.clone());

        let mut tree = Self {
            root,
            nodes: Vec::new(),
            selected: 0,
            expanded,
            create_input: String::new(),
        };

        tree.refresh()?;
        Ok(tree)
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.nodes.clear();

        self.push_children(self.root.clone(), 0)?;

        if self.nodes.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.nodes.len() {
            self.selected = self.nodes.len() - 1;
        }

        Ok(())
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.nodes.is_empty() {
            self.selected = 0;
            return;
        }

        let max = self.nodes.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max);
        self.selected = next as usize;
    }

    pub fn selected_node(&self) -> Option<&FileNode> {
        self.nodes.get(self.selected)
    }

    pub fn all_file_paths(&self) -> Vec<PathBuf> {
        WalkBuilder::new(&self.root)
            .hidden(false)
            .build()
            .flatten()
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                if metadata.is_file() {
                    Some(entry.path().to_path_buf())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    pub fn toggle_selected_dir(&mut self) -> Result<()> {
        let Some(node) = self.selected_node() else {
            return Ok(());
        };

        if !node.is_dir {
            return Ok(());
        }

        let path = node.path.clone();

        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }

        self.refresh()
    }

    pub fn collapse_selected_or_parent(&mut self) -> Result<()> {
        let Some(node) = self.selected_node().cloned() else {
            return Ok(());
        };

        if node.is_dir && self.expanded.contains(&node.path) {
            self.expanded.remove(&node.path);
            return self.refresh();
        }

        let Some(parent) = node.path.parent() else {
            return Ok(());
        };

        if let Some((idx, _)) = self
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.path == parent)
        {
            self.selected = idx;
        }

        Ok(())
    }

    pub fn begin_create(&mut self) {
        self.create_input.clear();
    }

    pub fn create_target_base_dir(&self) -> PathBuf {
        match self.selected_node() {
            Some(node) if node.is_dir => node.path.clone(),
            Some(node) => node
                .path
                .parent()
                .map_or_else(|| self.root.clone(), Path::to_path_buf),
            None => self.root.clone(),
        }
    }

    pub fn commit_create(&mut self) -> Result<Option<PathBuf>> {
        let input = self.create_input.trim();
        if input.is_empty() {
            return Ok(None);
        }

        let base = self.create_target_base_dir();
        let mut target = base.join(input);

        if input.ends_with('/') {
            std::fs::create_dir_all(&target)?;
            self.expanded.insert(target.clone());
            self.create_input.clear();
            self.refresh()?;
            return Ok(None);
        }

        if target.extension().is_none() {
            target.set_extension("md");
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if !target.exists() {
            let title = target
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled");
            std::fs::write(&target, format!("# {title}\n\n"))?;
        }

        self.create_input.clear();
        self.refresh()?;
        Ok(Some(target))
    }

    fn push_children(&mut self, dir: PathBuf, depth: usize) -> Result<()> {
        let mut entries: Vec<(PathBuf, bool, String)> = WalkBuilder::new(&dir)
            .max_depth(Some(1))
            .hidden(false)
            .build()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path().to_path_buf();
                if path == dir {
                    return None;
                }

                let metadata = entry.metadata().ok()?;
                let is_dir = metadata.is_dir();
                let name = entry.file_name().to_str()?.to_string();
                Some((path, is_dir, name))
            })
            .collect();

        entries.sort_by(|a, b| match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.2.to_lowercase().cmp(&b.2.to_lowercase()),
        });

        for (path, is_dir, name) in entries {
            self.nodes.push(FileNode {
                path: path.clone(),
                name,
                depth,
                is_dir,
            });

            if is_dir && self.expanded.contains(&path) {
                self.push_children(path, depth + 1)?;
            }
        }

        Ok(())
    }
}
