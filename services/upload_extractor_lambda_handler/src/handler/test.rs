use super::*;
use model::document::FileType;
use model::folder::FolderItem;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

#[tokio::test]
async fn test_build_file_system() {
    // Create a temporary directory for testing
    let temp_dir = tempdir().unwrap();
    let extract_dir = temp_dir.path();

    // Create a test file structure
    let root_dir = extract_dir.join("root");
    fs::create_dir_all(&root_dir).unwrap();

    // Create a file in the root directory
    let root_file_path = root_dir.join("root_file.txt");
    let mut root_file = File::create(&root_file_path).unwrap();
    root_file.write_all(b"root file content").unwrap();

    // Create a subdirectory
    let subdir_path = root_dir.join("subdir");
    fs::create_dir_all(&subdir_path).unwrap();

    // Create a file in the subdirectory
    let subdir_file_path = subdir_path.join("subdir_file.pdf");
    let mut subdir_file = File::create(&subdir_file_path).unwrap();
    subdir_file.write_all(b"subdir file content").unwrap();

    // Create a nested subdirectory
    let nested_subdir_path = subdir_path.join("nested");
    fs::create_dir_all(&nested_subdir_path).unwrap();

    // Create a file in the nested subdirectory
    let nested_file_path = nested_subdir_path.join("nested_file.docx");
    let mut nested_file = File::create(&nested_file_path).unwrap();
    nested_file.write_all(b"nested file content").unwrap();

    // Run the build_file_system function
    let result = build_file_system(extract_dir).await.unwrap().items;

    // Verify the results
    assert_eq!(result.len(), 3); // Should have 3 files total

    // Check that each file has the correct relative path and name
    let root_file_item = result.iter().find(|item| item.name == "root_file").unwrap();
    assert_eq!(root_file_item.relative_path, "root");
    assert_eq!(root_file_item.file_type, Some(FileType::Txt));

    let subdir_file_item = result
        .iter()
        .find(|item| item.name == "subdir_file")
        .unwrap();
    assert_eq!(subdir_file_item.relative_path, "root/subdir");
    assert_eq!(subdir_file_item.file_type, Some(FileType::Pdf));

    let nested_file_item = result
        .iter()
        .find(|item| item.name == "nested_file")
        .unwrap();
    assert_eq!(nested_file_item.relative_path, "root/subdir/nested");
    assert_eq!(nested_file_item.file_type, Some(FileType::Docx));
}

#[test]
fn test_get_file_path_for_item_with_file_type() {
    let extract_dir = PathBuf::from("/tmp/extract");
    let item = FolderItem {
        name: "document".to_string(),
        full_name: "document.pdf".to_string(),
        file_type: Some(FileType::Pdf),
        relative_path: "folder/subfolder".to_string(),
        sha: "abcd1234".to_string(),
    };

    let path = get_file_path_for_item(&extract_dir, &item);
    assert_eq!(
        path,
        PathBuf::from("/tmp/extract/folder/subfolder/document.pdf")
    );
}

#[test]
fn test_get_file_path_for_item_with_multi_part_extension() {
    let extract_dir = PathBuf::from("/tmp/extract");
    let item = FolderItem {
        name: "document".to_string(),
        full_name: "document.tar.gz".to_string(),
        file_type: Some(FileType::TarGz),
        relative_path: "folder/subfolder".to_string(),
        sha: "abcd1234".to_string(),
    };

    let path = get_file_path_for_item(&extract_dir, &item);
    assert_eq!(
        path,
        PathBuf::from("/tmp/extract/folder/subfolder/document.tar.gz")
    );
}

#[test]
fn test_get_file_path_for_item_without_file_type() {
    let extract_dir = PathBuf::from("/tmp/extract");
    let item = FolderItem {
        name: "config".to_string(),
        full_name: "config".to_string(),
        file_type: None,
        relative_path: "settings".to_string(),
        sha: "efgh5678".to_string(),
    };

    let path = get_file_path_for_item(&extract_dir, &item);
    assert_eq!(path, PathBuf::from("/tmp/extract/settings/config"));
}

#[test]
fn test_get_file_path_for_item_with_periods_in_name() {
    let extract_dir = PathBuf::from("/tmp/extract");
    let item = FolderItem {
        name: "test.1.5.5".to_string(),
        full_name: "test.1.5.5.tar.gz".to_string(),
        file_type: Some(FileType::TarGz),
        relative_path: "settings".to_string(),
        sha: "efgh5678".to_string(),
    };

    let path = get_file_path_for_item(&extract_dir, &item);
    assert_eq!(
        path,
        PathBuf::from("/tmp/extract/settings/test.1.5.5.tar.gz")
    );
}

#[test]
fn test_get_file_path_for_item_with_empty_relative_path() {
    let extract_dir = PathBuf::from("/tmp/extract");
    let item = FolderItem {
        name: "root_file".to_string(),
        full_name: "root_file.txt".to_string(),
        file_type: Some(FileType::Txt),
        relative_path: "".to_string(),
        sha: "ijkl9012".to_string(),
    };

    let path = get_file_path_for_item(&extract_dir, &item);
    assert_eq!(path, PathBuf::from("/tmp/extract/root_file.txt"));
}

#[tokio::test]
async fn test_build_file_system_handles_empty_directory() {
    // Create an empty temporary directory
    let temp_dir = tempdir().unwrap();
    let extract_dir = temp_dir.path();

    // Run the build_file_system function
    let result = build_file_system(extract_dir).await.unwrap().items;

    // Verify the results
    assert_eq!(result.len(), 0); // Should be empty
    let tree = build_file_system(extract_dir).await.unwrap();
    assert!(tree.folders.is_empty());
}

#[tokio::test]
async fn test_build_file_system_skips_hidden_files() {
    // Create a temporary directory
    let temp_dir = tempdir().unwrap();
    let extract_dir = temp_dir.path();

    let root_dir = extract_dir.join("root");
    fs::create_dir_all(&root_dir).unwrap();

    // Create a visible file
    let visible_file_path = root_dir.join("visible.txt");
    let mut visible_file = File::create(&visible_file_path).unwrap();
    visible_file.write_all(b"visible content").unwrap();

    // Create a hidden file
    let hidden_file_path = root_dir.join(".hidden.txt");
    let mut hidden_file = File::create(&hidden_file_path).unwrap();
    hidden_file.write_all(b"hidden content").unwrap();

    // Run the build_file_system function
    let result = build_file_system(extract_dir).await.unwrap().items;

    // Verify that only the visible file is included
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "visible");
    assert_eq!(result[0].file_type, Some(FileType::Txt));
}

#[tokio::test]
async fn test_build_file_system_handles_file_without_extension() {
    // Create a temporary directory
    let temp_dir = tempdir().unwrap();
    let extract_dir = temp_dir.path();

    // Create a file without extension
    let file_path = extract_dir.join("no_extension");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"content without extension").unwrap();

    // Run the build_file_system function
    let result = build_file_system(extract_dir).await.unwrap().items;

    // Verify the results
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "no_extension");
    assert_eq!(result[0].file_type, None);
}

#[tokio::test]
async fn test_build_file_system_computes_correct_sha() {
    // Create a temporary directory
    let temp_dir = tempdir().unwrap();
    let extract_dir = temp_dir.path();

    // Create a file with known content for SHA verification
    let file_path = extract_dir.join("test.txt");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"test content for SHA").unwrap();

    // Manually compute the SHA256 hash for comparison
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"test content for SHA");
    let expected_sha = format!("{:x}", hasher.finalize());

    // Run the build_file_system function
    let result = build_file_system(extract_dir).await.unwrap().items;

    // Verify the SHA256 hash
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].sha, expected_sha);
}

#[test]
fn test_get_file_path_for_item_keeps_extension_case() {
    let extract_dir = PathBuf::from("/tmp/extract");
    let item = FolderItem {
        name: "REPORT".to_string(),
        full_name: "REPORT.DOCX".to_string(),
        file_type: Some(FileType::Docx),
        relative_path: "Root".to_string(),
        sha: "abcd".to_string(),
    };

    assert_eq!(
        get_file_path_for_item(&extract_dir, &item),
        PathBuf::from("/tmp/extract/Root/REPORT.DOCX")
    );
}

#[test]
fn test_is_ignored_entry() {
    for ignored in [
        "Root/.DS_Store",
        "Root/~$Summary.docx",
        "Root/Sub/~$Deck.pptx",
        "Root/Thumbs.db",
        "Root/THUMBS.DB",
        "Root/desktop.ini",
        "Root/Desktop.ini",
        "Root/ehthumbs.db",
        "Root/Icon\r",
        "Root/.git/config",
        "Root/__MACOSX/._a.docx",
        "Root/Sub/.~lock.Budget.xlsx#",
    ] {
        assert!(is_ignored_entry(Path::new(ignored)), "{ignored}");
    }
    for kept in [
        "Root",
        ".dotted-root",
        ".dotted-root/a.docx",
        "Root/a.docx",
        "Root/Sub/b.pptx",
        "Root/Thumbs.db.xlsx",
        "Root/report~$.docx",
        "Root/Icon.png",
    ] {
        assert!(!is_ignored_entry(Path::new(kept)), "{kept}");
    }
}

/// Zip entries as the web app's zip worker writes them for a dropped folder.
fn write_browser_zip(path: &Path, entries: &[(&str, Option<&[u8]>)]) {
    let file = File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, contents) in entries {
        match contents {
            Some(bytes) => {
                zip.start_file(*name, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
            None => zip.add_directory(*name, options).unwrap(),
        }
    }
    zip.finish().unwrap();
}

async fn extract_tree(
    entries: &[(&str, Option<&[u8]>)],
    zip_name: &str,
) -> (String, ExtractedTree, PathBuf, tempfile::TempDir) {
    let temp_dir = tempdir().unwrap();
    let zip_path = temp_dir.path().join("upload.zip");
    write_browser_zip(&zip_path, entries);
    let extract_dir = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_dir).unwrap();
    let root = extract_zip_file(&zip_path, &extract_dir, Some(zip_name)).unwrap();
    let tree = build_file_system(&extract_dir).await.unwrap();
    (root, tree, extract_dir, temp_dir)
}

#[tokio::test]
async fn test_nested_office_folder_upload() {
    let entries: &[(&str, Option<&[u8]>)] = &[
        ("Board Pack/", None),
        ("Board Pack/Summary.docx", Some(b"docx")),
        ("Board Pack/REPORT UPPER.DOCX", Some(b"upper")),
        ("Board Pack/Deck.PPTX", Some(b"upper pptx")),
        ("Board Pack/Café Menü.docx", Some(b"unicode")),
        ("Board Pack/~$Summary.docx", Some(b"lock")),
        ("Board Pack/.DS_Store", Some(b"ds")),
        ("Board Pack/Thumbs.db", Some(b"thumbs")),
        ("Board Pack/desktop.ini", Some(b"ini")),
        ("Board Pack/.git/", None),
        ("Board Pack/.git/config", Some(b"git")),
        ("Board Pack/Empty Folder/", None),
        ("Board Pack/Finance/", None),
        ("Board Pack/Finance/Budget.xlsx", Some(b"xlsx")),
        ("Board Pack/Finance/Forecasts/", None),
        ("Board Pack/Finance/Forecasts/Scenarios/", None),
        (
            "Board Pack/Finance/Forecasts/Scenarios/Downside.xlsx",
            Some(b"xlsx2"),
        ),
        ("Board Pack/Finance/Forecasts/Scenarios/Empty Leaf/", None),
        ("Board Pack/Decks/", None),
        ("Board Pack/Decks/Archive/", None),
        ("Board Pack/Decks/Archive/Old.pptx", Some(b"pptx")),
        ("Board Pack/Decks/Archive/~$Old.pptx", Some(b"lock")),
    ];
    let (root, tree, extract_dir, _guard) = extract_tree(entries, "Board Pack").await;
    assert_eq!(root, "Board Pack");

    let mut files: Vec<String> = tree
        .items
        .iter()
        .map(|item| format!("{}/{}", item.relative_path, item.full_name))
        .collect();
    files.sort();
    assert_eq!(
        files,
        vec![
            "Board Pack/Café Menü.docx",
            "Board Pack/Deck.PPTX",
            "Board Pack/Decks/Archive/Old.pptx",
            "Board Pack/Finance/Budget.xlsx",
            "Board Pack/Finance/Forecasts/Scenarios/Downside.xlsx",
            "Board Pack/REPORT UPPER.DOCX",
            "Board Pack/Summary.docx",
        ]
    );

    // Every kept file must be found again when it is copied to storage.
    for item in &tree.items {
        assert!(
            get_file_path_for_item(&extract_dir, item).is_file(),
            "{item:?}"
        );
    }
    let upper = tree
        .items
        .iter()
        .find(|item| item.full_name == "REPORT UPPER.DOCX")
        .unwrap();
    assert_eq!(upper.file_type, Some(FileType::Docx));
    assert_eq!(upper.name, "REPORT UPPER");
    let deck = tree
        .items
        .iter()
        .find(|item| item.full_name == "Deck.PPTX")
        .unwrap();
    assert_eq!(deck.file_type, Some(FileType::Pptx));

    assert_eq!(
        tree.folders,
        vec![
            "Board Pack",
            "Board Pack/Decks",
            "Board Pack/Decks/Archive",
            "Board Pack/Empty Folder",
            "Board Pack/Finance",
            "Board Pack/Finance/Forecasts",
            "Board Pack/Finance/Forecasts/Scenarios",
            "Board Pack/Finance/Forecasts/Scenarios/Empty Leaf",
        ]
    );
}

#[tokio::test]
async fn test_folder_with_only_empty_subfolders() {
    let entries: &[(&str, Option<&[u8]>)] =
        &[("Shell/", None), ("Shell/A/", None), ("Shell/A/B/", None)];
    let (root, tree, _extract_dir, _guard) = extract_tree(entries, "Shell").await;
    assert_eq!(root, "Shell");
    assert!(tree.items.is_empty());
    assert_eq!(tree.folders, vec!["Shell", "Shell/A", "Shell/A/B"]);
}

#[tokio::test]
async fn test_single_subfolder_keeps_picked_root() {
    // The picked folder holds one subfolder and no files of its own.
    let entries: &[(&str, Option<&[u8]>)] = &[
        ("Wrapper/", None),
        ("Wrapper/Inner/", None),
        ("Wrapper/Inner/a.docx", Some(b"a")),
    ];
    let (root, tree, _extract_dir, _guard) = extract_tree(entries, "Wrapper").await;
    assert_eq!(root, "Wrapper");
    assert_eq!(tree.items.len(), 1);
    assert_eq!(tree.items[0].relative_path, "Wrapper/Inner");
    assert_eq!(tree.folders, vec!["Wrapper", "Wrapper/Inner"]);
}
