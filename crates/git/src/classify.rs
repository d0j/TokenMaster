use tokenmaster_domain::GitOutputCategory;

use crate::{GitCoreError, MAX_GIT_PATH_BYTES};

pub fn classify_destination_path(path: &[u8]) -> Result<GitOutputCategory, GitCoreError> {
    let normalized = normalize_relative_path(path)?;
    let components = normalized
        .split(|byte| *byte == b'/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    let Some(file_name) = components.last().copied() else {
        return Err(GitCoreError::InvalidPath);
    };

    if components
        .iter()
        .any(|component| is_one_of(component, VENDOR_COMPONENTS))
        || is_one_of(file_name, GENERATED_FILES)
        || GENERATED_SUFFIXES
            .iter()
            .any(|suffix| file_name.ends_with(suffix))
    {
        return Ok(GitOutputCategory::VendorGenerated);
    }
    if components
        .iter()
        .any(|component| is_one_of(component, SCHEMA_COMPONENTS))
        || is_schema_file(file_name)
    {
        return Ok(GitOutputCategory::SchemaMigration);
    }
    if components
        .iter()
        .any(|component| is_one_of(component, TEST_COMPONENTS))
        || is_test_file(file_name)
    {
        return Ok(GitOutputCategory::Test);
    }
    if components
        .iter()
        .any(|component| is_one_of(component, DOC_COMPONENTS))
        || extension(file_name).is_some_and(|ext| is_one_of(ext, DOC_EXTENSIONS))
        || is_one_of(file_name, DOC_FILES)
    {
        return Ok(GitOutputCategory::DocsSpec);
    }
    if components
        .iter()
        .any(|component| is_one_of(component, CONFIG_COMPONENTS))
        || is_one_of(file_name, CONFIG_FILES)
        || extension(file_name).is_some_and(|ext| is_one_of(ext, CONFIG_EXTENSIONS))
    {
        return Ok(GitOutputCategory::ConfigBuild);
    }
    if extension(file_name).is_some_and(|ext| is_one_of(ext, ASSET_EXTENSIONS)) {
        return Ok(GitOutputCategory::Asset);
    }
    if extension(file_name).is_some_and(|ext| is_one_of(ext, SOURCE_EXTENSIONS))
        || is_one_of(file_name, SOURCE_FILES)
    {
        return Ok(GitOutputCategory::ProductCode);
    }
    Ok(GitOutputCategory::Other)
}

fn normalize_relative_path(path: &[u8]) -> Result<Vec<u8>, GitCoreError> {
    if path.is_empty()
        || path.len() > MAX_GIT_PATH_BYTES
        || path[0] == b'/'
        || path[0] == b'\\'
        || (path.len() >= 2 && path[0].is_ascii_alphabetic() && path[1] == b':')
        || path.contains(&0)
    {
        return Err(GitCoreError::InvalidPath);
    }
    let mut normalized = Vec::with_capacity(path.len());
    for byte in path {
        normalized.push(if *byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        });
    }
    if normalized
        .split(|byte| *byte == b'/')
        .any(|component| component == b"." || component == b"..")
    {
        return Err(GitCoreError::InvalidPath);
    }
    Ok(normalized)
}

fn extension(file_name: &[u8]) -> Option<&[u8]> {
    let position = file_name.iter().rposition(|byte| *byte == b'.')?;
    if position + 1 == file_name.len() {
        return None;
    }
    Some(&file_name[position + 1..])
}

fn is_one_of(value: &[u8], values: &[&[u8]]) -> bool {
    values.contains(&value)
}

fn is_test_file(file_name: &[u8]) -> bool {
    file_name.windows(6).any(|window| window == b".test.")
        || file_name.windows(6).any(|window| window == b"_test.")
        || file_name.windows(6).any(|window| window == b".spec.")
        || file_name.windows(6).any(|window| window == b"_spec.")
}

fn is_schema_file(file_name: &[u8]) -> bool {
    file_name.starts_with(b"migration")
        || file_name.starts_with(b"schema.")
        || file_name.ends_with(b"_migration.sql")
}

const VENDOR_COMPONENTS: &[&[u8]] = &[
    b"vendor",
    b"third_party",
    b"node_modules",
    b"generated",
    b"dist",
    b"target",
    b".cache",
];
const GENERATED_FILES: &[&[u8]] = &[
    b"cargo.lock",
    b"package-lock.json",
    b"pnpm-lock.yaml",
    b"yarn.lock",
    b"composer.lock",
    b"poetry.lock",
    b"uv.lock",
];
const GENERATED_SUFFIXES: &[&[u8]] = &[b".min.js", b".min.css", b".map"];
const SCHEMA_COMPONENTS: &[&[u8]] = &[b"migrations", b"migration", b"schema", b"schemas"];
const TEST_COMPONENTS: &[&[u8]] = &[
    b"test",
    b"tests",
    b"spec",
    b"specs",
    b"__tests__",
    b"fixtures",
];
const DOC_COMPONENTS: &[&[u8]] = &[b"doc", b"docs", b"documentation"];
const DOC_EXTENSIONS: &[&[u8]] = &[b"md", b"mdx", b"adoc", b"asciidoc", b"rst"];
const DOC_FILES: &[&[u8]] = &[
    b"readme",
    b"readme.md",
    b"changelog",
    b"changelog.md",
    b"specification.md",
];
const CONFIG_COMPONENTS: &[&[u8]] = &[
    b".github",
    b".gitlab",
    b".circleci",
    b"ci",
    b"config",
    b"configs",
];
const CONFIG_FILES: &[&[u8]] = &[
    b"cargo.toml",
    b"package.json",
    b"dockerfile",
    b"makefile",
    b"cmakelists.txt",
    b"justfile",
    b"build.rs",
    b"rust-toolchain.toml",
];
const CONFIG_EXTENSIONS: &[&[u8]] = &[
    b"toml", b"yaml", b"yml", b"json", b"json5", b"ini", b"cfg", b"conf", b"xml",
];
const ASSET_EXTENSIONS: &[&[u8]] = &[
    b"png", b"jpg", b"jpeg", b"gif", b"webp", b"ico", b"svg", b"woff", b"woff2", b"ttf", b"otf",
    b"mp3", b"wav", b"ogg", b"mp4", b"webm", b"zip", b"7z", b"rar", b"pdf",
];
const SOURCE_EXTENSIONS: &[&[u8]] = &[
    b"rs", b"c", b"h", b"cc", b"cpp", b"cxx", b"hpp", b"cs", b"go", b"java", b"kt", b"kts",
    b"swift", b"m", b"mm", b"py", b"pyi", b"rb", b"php", b"js", b"jsx", b"ts", b"tsx", b"vue",
    b"svelte", b"scala", b"sh", b"ps1", b"sql", b"proto", b"graphql", b"gql", b"slint", b"html",
    b"css", b"scss", b"sass", b"less", b"lua", b"zig", b"ex", b"exs", b"erl", b"hrl", b"fs",
    b"fsx", b"vb",
];
const SOURCE_FILES: &[&[u8]] = &[b"meson.build", b"build.gradle", b"build.gradle.kts"];
