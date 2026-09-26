use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

/// Canonicalize both sides before deleting, including any symlinked parent directories.
pub(super) fn delete_attachment_in_dir(root: &Path, target: &Path) -> Result<bool, String> {
    let denied = || "不允许删除附件目录之外的文件".to_string();
    if !target.is_absolute() || target.components().any(|part| part == Component::ParentDir) {
        return Err(denied());
    }
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Err(denied()),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("读取附件目录失败: {error}")),
    }
    let root = match root.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("读取附件目录失败: {error}")),
    };
    let target = match target.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("读取附件路径失败: {error}")),
    };
    if target == root || !target.starts_with(&root) {
        return Err(denied());
    }
    match fs::remove_file(target) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("删除附件文件失败: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletes_only_the_requested_attachment_and_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("attachments");
        fs::create_dir(&root).unwrap();
        let target = root.join("image.png");
        let other = root.join("other.png");
        fs::write(&target, b"image").unwrap();
        fs::write(&other, b"other").unwrap();

        assert_eq!(delete_attachment_in_dir(&root, &target), Ok(true));
        assert_eq!(delete_attachment_in_dir(&root, &target), Ok(false));
        assert!(other.exists());
    }

    #[test]
    fn rejects_parent_traversal_and_similar_directory_prefix() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("attachments");
        let other_root = directory.path().join("attachments-other");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&other_root).unwrap();
        let outside = directory.path().join("config.json");
        let other = other_root.join("image.png");
        fs::write(&outside, b"config").unwrap();
        fs::write(&other, b"image").unwrap();

        assert!(delete_attachment_in_dir(&root, &root.join("../config.json")).is_err());
        assert!(delete_attachment_in_dir(&root, &other).is_err());
        assert!(delete_attachment_in_dir(&root, &root).is_err());
        assert!(outside.exists());
        assert!(other.exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_to_external_file_and_parent_directory() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("attachments");
        fs::create_dir(&root).unwrap();
        let outside = directory.path().join("config.json");
        fs::write(&outside, b"config").unwrap();
        symlink(&outside, root.join("image.png")).unwrap();
        symlink(directory.path(), root.join("linked")).unwrap();

        assert!(delete_attachment_in_dir(&root, &root.join("image.png")).is_err());
        assert!(delete_attachment_in_dir(&root, &root.join("linked/config.json")).is_err());
        assert!(outside.exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_attachment_root_replaced_with_symlink() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = directory.path().join("attachments");
        let protected = outside.path().join("config.json");
        fs::write(&protected, b"protected").unwrap();
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();
        assert!(delete_attachment_in_dir(&root, &root.join("config.json")).is_err());
        assert_eq!(fs::read(protected).unwrap(), b"protected");
    }

    #[test]
    fn missing_attachment_directory_is_a_noop() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("attachments");
        assert_eq!(
            delete_attachment_in_dir(&root, &root.join("image.png")),
            Ok(false)
        );
    }
}
