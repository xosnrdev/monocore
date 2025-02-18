use monoutils_store::IpldStore;
use typed_path::Utf8UnixPath;

use crate::{
    filesystem::{dir::find, entity::Entity, file::File, EntityCidLink, FsError, FsResult},
    utils::path,
};

use super::{Dir, FindResult, Utf8UnixPathSegment};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Directory operations.
impl<S> Dir<S>
where
    S: IpldStore + Send + Sync,
{
    /// Finds an entity in the directory structure given a path.
    ///
    /// This method traverses the directory structure to find the entity specified by the path.
    /// It returns a reference to the found entity if it exists.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity, FsResult};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> FsResult<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo/bar.txt", true).await?;
    ///
    /// let entity = dir.find("foo/bar.txt").await?;
    /// assert!(matches!(entity, Some(Entity::File(_))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find(&self, path: impl AsRef<str>) -> FsResult<Option<&Entity<S>>> {
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, file_name) = path::split_last(path)?;
        if let Some(parent_path) = parent {
            return match find::find_dir(self, parent_path).await? {
                FindResult::Found { dir } => dir.get_entity(&file_name).await,
                _ => Ok(None),
            };
        }

        self.get_entity(&file_name).await
    }

    /// Finds an entity in the directory structure given a path, returning a mutable reference.
    ///
    /// This method is similar to `find`, but it returns a mutable reference to the found entity.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity, FsResult};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> FsResult<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo/bar.txt", true).await?;
    ///
    /// let entity = dir.find_mut("foo/bar.txt").await?;
    /// assert!(matches!(entity, Some(Entity::File(_))));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find_mut(&mut self, path: impl AsRef<str>) -> FsResult<Option<&mut Entity<S>>> {
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, file_name) = path::split_last(path)?;
        if let Some(parent_path) = parent {
            return match find::find_dir_mut(self, parent_path).await? {
                FindResult::Found { dir } => dir.get_entity_mut(&file_name).await,
                _ => Ok(None),
            };
        }

        self.get_entity_mut(&file_name).await
    }

    /// Finds an entity in the directory structure or creates it if it doesn't exist.
    ///
    /// This method traverses the directory structure to find the entity specified by the path.
    /// If the entity doesn't exist, it creates a new file or directory based on the `file` parameter.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity, FsResult};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> FsResult<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    ///
    /// // Create a file
    /// let file = dir.find_or_create("foo/bar.txt", true).await?;
    /// assert!(matches!(file, Entity::File(_)));
    ///
    /// // Create a directory
    /// let subdir = dir.find_or_create("baz", false).await?;
    /// assert!(matches!(subdir, Entity::Dir(_)));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find_or_create(
        &mut self,
        path: impl AsRef<str>,
        file: bool,
    ) -> FsResult<&mut Entity<S>> {
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, file_name) = path::split_last(path)?;
        let parent_dir = match parent {
            Some(parent_path) => find::find_or_create_dir(self, parent_path).await?,
            None => self,
        };

        if parent_dir.has_entry(&file_name).await? {
            return parent_dir
                .get_entity_mut(&file_name)
                .await?
                .ok_or_else(|| FsError::PathNotFound(path.to_string()));
        }

        let new_entity = if file {
            Entity::File(File::new(parent_dir.get_store().clone()))
        } else {
            Entity::Dir(Dir::new(parent_dir.get_store().clone()))
        };

        parent_dir.put_entity(file_name.clone(), new_entity)?;

        parent_dir
            .get_entity_mut(&file_name)
            .await?
            .ok_or_else(|| FsError::PathNotFound(path.to_string()))
    }

    /// Lists all entries in the current directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, FsResult};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> FsResult<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo", false).await?;
    /// dir.find_or_create("bar.txt", true).await?;
    /// dir.find_or_create("baz/qux.txt", true).await?;
    ///
    /// let entries = dir.list()?;
    /// assert_eq!(entries.len(), 3);
    /// assert!(entries.contains(&"foo".parse()?));
    /// assert!(entries.contains(&"bar.txt".parse()?));
    /// assert!(entries.contains(&"baz".parse()?));
    /// # Ok(())
    /// # }
    /// ```
    pub fn list(&self) -> FsResult<Vec<Utf8UnixPathSegment>> {
        Ok(self.get_entries().map(|(k, _)| k.clone()).collect())
    }

    /// Copies an entity from the source path to the target **directory**.
    ///
    /// The target path must be a directory.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity, FsResult};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> FsResult<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("source/file.txt", true).await?;
    /// dir.find_or_create("target", false).await?;
    ///
    /// dir.copy("source/file.txt", "target").await?;
    ///
    /// assert!(dir.find("target/file.txt").await?.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn copy(&mut self, source: impl AsRef<str>, target: impl AsRef<str>) -> FsResult<()> {
        let source = Utf8UnixPath::new(source.as_ref());
        let target = Utf8UnixPath::new(target.as_ref());

        if source.has_root() || target.has_root() {
            return Err(FsError::PathHasRoot(source.to_string()));
        }

        let (source_parent, source_filename) = path::split_last(source)?;

        // Find source parent directory and entity
        let source_entity = if let Some(parent_path) = source_parent {
            let parent_dir = self
                .find(parent_path)
                .await?
                .and_then(|entity| {
                    if let Entity::Dir(dir) = entity {
                        Some(dir)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| FsError::SourceIsNotADir(parent_path.to_string()))?;

            parent_dir
                .get_entity(&source_filename)
                .await?
                .cloned()
                .ok_or_else(|| FsError::PathNotFound(source.to_string()))?
        } else {
            self.get_entity(&source_filename)
                .await?
                .cloned()
                .ok_or_else(|| FsError::PathNotFound(source.to_string()))?
        };

        // Find target directory
        let target_dir = match self.find_mut(target).await? {
            Some(Entity::Dir(dir)) => dir,
            _ => return Err(FsError::TargetIsNotADir(target.to_string())),
        };

        // Copy entity to target directory
        target_dir.put_entity(source_filename, source_entity)?;

        Ok(())
    }

    /// Removes an entity at the specified path and returns it.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity, FsResult};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> FsResult<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    /// dir.find_or_create("foo/bar.txt", true).await?;
    ///
    /// let (filename, _entity) = dir.remove("foo/bar.txt").await?;
    ///
    /// assert_eq!(filename, "bar.txt".parse()?);
    /// assert!(dir.find("foo/bar.txt").await?.is_none());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// TODO: Add support for tombstone files.
    pub async fn remove(
        &mut self,
        path: impl AsRef<str>,
    ) -> FsResult<(Utf8UnixPathSegment, EntityCidLink<S>)> {
        let path = Utf8UnixPath::new(path.as_ref());

        if path.has_root() {
            return Err(FsError::PathHasRoot(path.to_string()));
        }

        let (parent, filename) = path::split_last(path)?;

        let parent_dir = if let Some(parent_path) = parent {
            self.find_mut(parent_path)
                .await?
                .and_then(|entity| {
                    if let Entity::Dir(dir) = entity {
                        Some(dir)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| FsError::SourceIsNotADir(parent_path.to_string()))?
        } else {
            self
        };

        let entity = parent_dir.remove_entry(&filename)?;

        Ok((filename, entity))
    }

    /// Renames (moves) an entity from one path to another.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monofs::filesystem::{Dir, Entity, FsResult};
    /// use monoutils_store::MemoryStore;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> FsResult<()> {
    /// let mut dir = Dir::new(MemoryStore::default());
    ///
    /// // Create a file
    /// dir.find_or_create("old/file.txt", true).await?;
    /// dir.find_or_create("new", false).await?;
    ///
    /// // Rename the file
    /// dir.rename("old/file.txt", "new/file.txt").await?;
    ///
    /// assert!(dir.find("old/file.txt").await?.is_none());
    /// assert!(dir.find("new/file.txt").await?.is_some());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn rename(
        &mut self,
        old_path: impl AsRef<str>,
        new_path: impl AsRef<str>,
    ) -> FsResult<()> {
        let old_path = Utf8UnixPath::new(old_path.as_ref());
        let new_path = Utf8UnixPath::new(new_path.as_ref());

        if old_path.has_root() || new_path.has_root() {
            return Err(FsError::PathHasRoot(old_path.to_string()));
        }

        // Get the parent directories and filenames for both paths
        let (old_parent, old_filename) = path::split_last(old_path)?;
        let (new_parent, new_filename) = path::split_last(new_path)?;

        // First check if target exists to fail fast
        let target_exists = if let Some(parent_path) = new_parent {
            match find::find_dir(self, parent_path).await? {
                find::FindResult::Found { dir } => dir.has_entry(&new_filename).await?,
                _ => return Err(FsError::PathNotFound(new_path.to_string())),
            }
        } else {
            self.has_entry(&new_filename).await?
        };

        if target_exists {
            return Err(FsError::PathExists(new_path.to_string()));
        }

        // Get the source entity
        let source_entity = if let Some(parent_path) = old_parent {
            match find::find_dir_mut(self, parent_path).await? {
                find::FindResult::Found { dir } => dir.remove_entry(&old_filename)?,
                _ => return Err(FsError::PathNotFound(old_path.to_string())),
            }
        } else {
            self.remove_entry(&old_filename)?
        };

        // Store source_entity for potential rollback
        let source_entity_backup = source_entity.clone();

        // Get the target directory and try to put the entity there
        let result = if let Some(parent_path) = new_parent {
            match find::find_dir_mut(self, parent_path).await? {
                find::FindResult::Found { dir } => dir.put_entry(new_filename, source_entity),
                _ => Err(FsError::PathNotFound(new_path.to_string())),
            }
        } else {
            self.put_entry(new_filename, source_entity)
        };

        // If putting the entity in the target location fails, try to restore it to its original location
        if let Err(e) = result {
            if let Some(parent_path) = old_parent {
                if let find::FindResult::Found { dir } =
                    find::find_dir_mut(self, parent_path).await?
                {
                    dir.put_entry(old_filename, source_entity_backup)?;
                }
            } else {
                self.put_entry(old_filename, source_entity_backup)?;
            }
            return Err(e);
        }

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use monoutils_store::{MemoryStore, Storable};

    #[tokio::test]
    async fn test_ops_find() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a file and a subdirectory
        dir.find_or_create("foo/bar.txt", true).await?;
        dir.find_or_create("baz", false).await?;

        // Test finding existing entities
        assert!(matches!(
            dir.find("foo/bar.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(dir.find("baz").await?, Some(Entity::Dir(_))));

        // Test finding non-existent entity
        assert!(dir.find("nonexistent").await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_find_mut() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a file and a subdirectory
        dir.find_or_create("foo/bar.txt", true).await?;
        dir.find_or_create("baz", false).await?;

        // Test finding existing entities mutably
        assert!(matches!(
            dir.find_mut("foo/bar.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(dir.find_mut("baz").await?, Some(Entity::Dir(_))));

        // Test finding non-existent entity
        assert!(dir.find_mut("nonexistent").await?.is_none());

        // Test modifying a found file
        if let Some(Entity::File(file)) = dir.find_mut("foo/bar.txt").await? {
            let content = "Hello, World!".as_bytes();
            let content_cid = file.get_store().put_raw_block(content).await?;
            file.set_content(Some(content_cid));
            file.store().await?;
            assert_eq!(file.get_content(), Some(&content_cid));
        } else {
            panic!("Expected to find a file");
        }

        // Verify the modification persists
        if let Some(Entity::File(file)) = dir.find("foo/bar.txt").await? {
            let content_cid = file.get_content().expect("File should have content");
            let content = file.get_store().get_raw_block(content_cid).await?;
            assert_eq!(content, "Hello, World!".as_bytes());
        } else {
            panic!("Expected to find a file");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_find_or_create() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a file
        assert!(dir.find("foo/bar.txt").await?.is_none());
        let file = dir.find_or_create("foo/bar.txt", true).await?;
        assert!(matches!(file, Entity::File(_)));

        // Create a directory
        assert!(dir.find("baz").await?.is_none());
        let subdir = dir.find_or_create("baz", false).await?;
        assert!(matches!(subdir, Entity::Dir(_)));

        // Find existing entities
        let existing_file = dir.find("foo/bar.txt").await?;
        assert!(matches!(existing_file, Some(Entity::File(_))));

        let existing_dir = dir.find("baz").await?;
        assert!(matches!(existing_dir, Some(Entity::Dir(_))));

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_list() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create some entries
        dir.find_or_create("foo", false).await?;
        dir.find_or_create("bar.txt", true).await?;
        dir.find_or_create("baz/qux.txt", true).await?;

        // List entries
        let entries = dir.list()?;

        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&"foo".parse()?));
        assert!(entries.contains(&"bar.txt".parse()?));
        assert!(entries.contains(&"baz".parse()?));

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_copy() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create a source file
        assert!(dir.find("source/file.txt").await?.is_none());
        assert!(dir.find("target").await?.is_none());
        dir.find_or_create("source/file.txt", true).await?;
        dir.find_or_create("target", false).await?;

        // Copy the file
        assert!(dir.find("target/file.txt").await?.is_none());
        dir.copy("source/file.txt", "target").await?;

        // Verify the copy
        assert!(matches!(
            dir.find("source/file.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            dir.find("target/file.txt").await?,
            Some(Entity::File(_))
        ));

        // Test copying a directory
        assert!(dir.find("source/subdir").await?.is_none());
        dir.find_or_create("source/subdir", false).await?;

        assert!(dir.find("target/subdir").await?.is_none());
        dir.copy("source/subdir", "target").await?;

        assert!(matches!(
            dir.find("source/subdir").await?,
            Some(Entity::Dir(_))
        ));
        assert!(matches!(
            dir.find("target/subdir").await?,
            Some(Entity::Dir(_))
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_remove() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Create entities to remove
        dir.find_or_create("foo/bar.txt", true).await?;
        dir.find_or_create("baz", false).await?;

        // Remove file
        let (filename, entity) = dir.remove("foo/bar.txt").await?;
        assert_eq!(filename, "bar.txt".parse()?);
        assert!(matches!(entity, EntityCidLink::Decoded(Entity::File(_))));
        assert!(dir.find("foo/bar.txt").await?.is_none());
        assert!(dir.find("foo").await?.is_some());

        // Remove directory
        let (dirname, entity) = dir.remove("baz").await?;
        assert_eq!(dirname, "baz".parse()?);
        assert!(matches!(entity, EntityCidLink::Decoded(Entity::Dir(_))));
        assert!(dir.find("baz").await?.is_none());

        // Try to remove non-existent entity
        assert!(dir.remove("nonexistent").await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_complex_nested_hierarchy() -> FsResult<()> {
        let mut root = Dir::new(MemoryStore::default());

        // Create a complex nested structure
        root.find_or_create("projects/web/index.html", true).await?;
        root.find_or_create("projects/web/styles/main.css", true)
            .await?;
        root.find_or_create("projects/app/src/main.rs", true)
            .await?;
        root.find_or_create("documents/personal/notes.txt", true)
            .await?;
        root.find_or_create("documents/work/report.pdf", true)
            .await?;

        // Verify the structure
        assert!(matches!(root.find("projects").await?, Some(Entity::Dir(_))));
        assert!(matches!(
            root.find("projects/web/index.html").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            root.find("projects/app/src/main.rs").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            root.find("documents/work/report.pdf").await?,
            Some(Entity::File(_))
        ));

        // List contents of directories
        let root_contents = root.list()?;
        assert_eq!(root_contents.len(), 2);
        assert!(root_contents.contains(&"projects".parse()?));
        assert!(root_contents.contains(&"documents".parse()?));

        if let Some(Entity::Dir(projects_dir)) = root.find("projects").await? {
            let projects_contents = projects_dir.list()?;
            assert_eq!(projects_contents.len(), 2);
            assert!(projects_contents.contains(&"web".parse()?));
            assert!(projects_contents.contains(&"app".parse()?));
        } else {
            panic!("Expected to find 'projects' directory");
        }

        // Modify a file
        if let Some(Entity::File(index_file)) = root.find_mut("projects/web/index.html").await? {
            let content = "<html><body>Hello, World!</body></html>".as_bytes();
            let content_cid = index_file.get_store().put_raw_block(content).await?;
            index_file.set_content(Some(content_cid));
            index_file.store().await?;
        } else {
            panic!("Expected to find 'index.html' file");
        }

        // Verify the modification
        if let Some(Entity::File(index_file)) = root.find("projects/web/index.html").await? {
            let content_cid = index_file.get_content().expect("File should have content");
            let content = index_file.get_store().get_raw_block(content_cid).await?;
            assert_eq!(
                content,
                "<html><body>Hello, World!</body></html>".as_bytes()
            );
        } else {
            panic!("Expected to find 'index.html' file");
        }

        // Copy a file
        root.copy("documents/personal/notes.txt", "projects")
            .await?;
        assert!(matches!(
            root.find("projects/notes.txt").await?,
            Some(Entity::File(_))
        ));

        // Remove a file
        let (removed_filename, _) = root.remove("documents/work/report.pdf").await?;
        assert_eq!(removed_filename, "report.pdf".parse()?);
        assert!(root.find("documents/work/report.pdf").await?.is_none());

        // Remove a file and its parent directory
        root.remove("documents/personal/notes.txt").await?;
        root.remove("documents/personal").await?;
        assert!(root.find("documents/personal").await?.is_none());

        // Verify the final structure
        assert!(matches!(
            root.find("projects/web/index.html").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            root.find("projects/app/src/main.rs").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            root.find("projects/notes.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(root.find("documents/personal").await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_ops_rename() -> FsResult<()> {
        let mut dir = Dir::new(MemoryStore::default());

        // Test 1: Basic file rename to new location
        dir.find_or_create("source/file.txt", true).await?;
        dir.find_or_create("target", false).await?;
        dir.rename("source/file.txt", "target/newfile.txt").await?;
        assert!(dir.find("source/file.txt").await?.is_none());
        assert!(matches!(
            dir.find("target/newfile.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 2: Rename within same directory (just name change)
        dir.find_or_create("samedir/original.txt", true).await?;
        dir.rename("samedir/original.txt", "samedir/renamed.txt")
            .await?;
        assert!(dir.find("samedir/original.txt").await?.is_none());
        assert!(matches!(
            dir.find("samedir/renamed.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 3: Directory rename with nested content and verify content preservation
        dir.find_or_create("olddir/subdir/file1.txt", true).await?;
        dir.find_or_create("olddir/subdir/file2.txt", true).await?;

        // Add some content to verify it's preserved
        if let Some(Entity::File(file)) = dir.find_mut("olddir/subdir/file1.txt").await? {
            let content = "test content".as_bytes();
            let content_cid = file.get_store().put_raw_block(content).await?;
            file.set_content(Some(content_cid));
            file.store().await?;
        }

        dir.rename("olddir", "newdir").await?;

        // Verify structure and content preservation
        assert!(dir.find("olddir").await?.is_none());
        assert!(matches!(dir.find("newdir").await?, Some(Entity::Dir(_))));
        assert!(matches!(
            dir.find("newdir/subdir/file1.txt").await?,
            Some(Entity::File(_))
        ));
        assert!(matches!(
            dir.find("newdir/subdir/file2.txt").await?,
            Some(Entity::File(_))
        ));

        // Verify content was preserved
        if let Some(Entity::File(file)) = dir.find("newdir/subdir/file1.txt").await? {
            let content_cid = file.get_content().expect("File should have content");
            let content = file.get_store().get_raw_block(content_cid).await?;
            assert_eq!(content, "test content".as_bytes());
        }

        // Test 4: Rename across different directory depths
        dir.find_or_create("shallow/file.txt", true).await?;
        dir.find_or_create("deep/path/to/dir", false).await?;
        dir.rename("shallow/file.txt", "deep/path/to/dir/file.txt")
            .await?;
        assert!(dir.find("shallow/file.txt").await?.is_none());
        assert!(matches!(
            dir.find("deep/path/to/dir/file.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 5: Rename with special characters in filename
        dir.find_or_create("special/file-with-dashes.txt", true)
            .await?;
        dir.rename(
            "special/file-with-dashes.txt",
            "special/file with spaces.txt",
        )
        .await?;
        assert!(matches!(
            dir.find("special/file with spaces.txt").await?,
            Some(Entity::File(_))
        ));

        // Test 6: Verify error cases
        // Test 6.1: Rename to existing path
        dir.find_or_create("file1.txt", true).await?;
        dir.find_or_create("file2.txt", true).await?;
        assert!(dir.rename("file1.txt", "file2.txt").await.is_err());
        assert!(dir.find("file1.txt").await?.is_some()); // Original file should still exist
        assert!(dir.find("file2.txt").await?.is_some());

        // Test 6.2: Rename non-existent source
        assert!(dir.rename("nonexistent.txt", "newfile.txt").await.is_err());

        // Test 6.3: Rename to non-existent target directory
        assert!(dir
            .rename("file1.txt", "nonexistent/newfile.txt")
            .await
            .is_err());
        assert!(dir.find("file1.txt").await?.is_some()); // Original file should still exist

        // Test 6.4: Rename with root paths
        assert!(dir.rename("/file1.txt", "newfile.txt").await.is_err());
        assert!(dir.rename("file1.txt", "/newfile.txt").await.is_err());

        // Test 7: Store state verification
        // Create a complex rename scenario and verify store state
        dir.find_or_create("state_test/source/file.txt", true)
            .await?;
        // Create target directory first
        dir.find_or_create("state_test/target", false).await?;

        if let Some(Entity::File(file)) = dir.find_mut("state_test/source/file.txt").await? {
            let content = "store state test".as_bytes();
            let content_cid = file.get_store().put_raw_block(content).await?;
            file.set_content(Some(content_cid));
            file.store().await?;
        }

        dir.rename(
            "state_test/source/file.txt",
            "state_test/target/renamed.txt",
        )
        .await?;

        // Verify store state after rename
        if let Some(Entity::File(file)) = dir.find("state_test/target/renamed.txt").await? {
            let content_cid = file.get_content().expect("File should have content");
            let content = file.get_store().get_raw_block(content_cid).await?;
            assert_eq!(content, "store state test".as_bytes());
        }

        Ok(())
    }
}
