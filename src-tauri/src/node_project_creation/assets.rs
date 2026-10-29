//! Reviewed local assets. Rendering is pure; it performs no IO or execution.
use super::{CreationPlan, Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntendedFile {
    pub bytes: u64,
    pub sha256: String,
}
pub fn revision() -> String {
    let mut bytes = include_bytes!("assets.rs").to_vec();
    for id in [
        "node-js",
        "node-ts",
        "hono-ts",
        "express-js",
        "react-vite-js",
        "react-vite-ts",
    ] {
        for (path, content) in sources(id).expect("known templates") {
            bytes.extend_from_slice(path.as_bytes());
            bytes.extend_from_slice(content.as_bytes());
        }
    }
    bytes.extend_from_slice(include_bytes!("../../../LICENSE"));
    bytes.extend_from_slice(super::locks::revision().as_bytes());
    super::digest(&bytes)
}
fn sources(id: &str) -> Result<&'static [(&'static str, &'static str)]> {
    Ok(match id {
        "node-js" => &[
            (
                "src/server.js",
                include_str!("../../templates/node-projects/node-js/src/server.js"),
            ),
            (
                "src/index.js",
                include_str!("../../templates/node-projects/node-js/src/index.js"),
            ),
            (
                "test/server.test.js",
                include_str!("../../templates/node-projects/node-js/test/server.test.js"),
            ),
        ],
        "node-ts" => &[
            (
                "src/index.ts",
                include_str!("../../templates/node-projects/node-ts/src/index.ts"),
            ),
            (
                "tsconfig.json",
                include_str!("../../templates/node-projects/node-ts/tsconfig.json"),
            ),
        ],
        "hono-ts" => &[
            (
                "src/index.ts",
                include_str!("../../templates/node-projects/hono-ts/src/index.ts"),
            ),
            (
                "tsconfig.json",
                include_str!("../../templates/node-projects/hono-ts/tsconfig.json"),
            ),
        ],
        "express-js" => &[
            (
                "src/app.js",
                include_str!("../../templates/node-projects/express-js/src/app.js"),
            ),
            (
                "src/index.js",
                include_str!("../../templates/node-projects/express-js/src/index.js"),
            ),
            (
                "test/api.test.js",
                include_str!("../../templates/node-projects/express-js/test/api.test.js"),
            ),
        ],
        "react-vite-js" => &[
            (
                "index.html",
                include_str!("../../templates/node-projects/react-vite-js/index.html"),
            ),
            (
                "src/main.jsx",
                include_str!("../../templates/node-projects/react-vite-js/src/main.jsx"),
            ),
            (
                "src/App.jsx",
                include_str!("../../templates/node-projects/react-vite-js/src/App.jsx"),
            ),
            (
                "src/index.css",
                include_str!("../../templates/node-projects/react-vite-js/src/index.css"),
            ),
            (
                "vite.config.js",
                include_str!("../../templates/node-projects/react-vite-js/vite.config.js"),
            ),
        ],
        "react-vite-ts" => &[
            (
                "index.html",
                include_str!("../../templates/node-projects/react-vite-ts/index.html"),
            ),
            (
                "src/main.tsx",
                include_str!("../../templates/node-projects/react-vite-ts/src/main.tsx"),
            ),
            (
                "src/App.tsx",
                include_str!("../../templates/node-projects/react-vite-ts/src/App.tsx"),
            ),
            (
                "src/index.css",
                include_str!("../../templates/node-projects/react-vite-ts/src/index.css"),
            ),
            (
                "vite.config.ts",
                include_str!("../../templates/node-projects/react-vite-ts/vite.config.ts"),
            ),
            (
                "tsconfig.json",
                include_str!("../../templates/node-projects/react-vite-ts/tsconfig.json"),
            ),
            (
                "tsconfig.app.json",
                include_str!("../../templates/node-projects/react-vite-ts/tsconfig.app.json"),
            ),
            (
                "tsconfig.node.json",
                include_str!("../../templates/node-projects/react-vite-ts/tsconfig.node.json"),
            ),
        ],
        _ => {
            return Err(Error::new(
                ErrorCode::InvalidTemplate,
                "Unknown local template assets.",
            ))
        }
    })
}
pub fn validate_entry(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 256
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.len() > 80)
    {
        return Err(Error::new(
            ErrorCode::UnsafeTemplate,
            "Template entries must be bounded relative file paths without traversal or links.",
        ));
    }
    Ok(())
}
pub fn describe(files: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, IntendedFile> {
    files
        .iter()
        .map(|(name, bytes)| {
            (
                name.clone(),
                IntendedFile {
                    bytes: bytes.len() as u64,
                    sha256: super::digest(bytes),
                },
            )
        })
        .collect()
}
pub fn render(plan: &CreationPlan) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (path, content) in sources(&plan.template.id)? {
        files.insert(
            (*path).into(),
            content
                .replace("__PROJECT_NAME__", &plan.project_name)
                .into_bytes(),
        );
    }
    let mut dependencies = BTreeMap::new();
    let mut dev_dependencies = BTreeMap::new();
    for dependency in &plan.template.dependencies {
        if dependency.development {
            dev_dependencies.insert(&dependency.name, &dependency.version);
        } else {
            dependencies.insert(&dependency.name, &dependency.version);
        }
    }
    let package = serde_json::json!({"name":plan.project_name,"version":"0.1.0","private":true,"type":"module","license":"MIT",
        "engines":{"node":plan.template.node_range,"npm":plan.template.package_manager.version_range},"packageManager":format!("npm@{}",plan.tools.npm_version),
        "scripts":plan.template.scripts,"dependencies":dependencies,"devDependencies":dev_dependencies});
    let json = |value: &serde_json::Value| -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(value).expect("serializable assets");
        bytes.push(b'\n');
        bytes
    };
    files.insert("package.json".into(), json(&package));
    files.insert(
        "package-lock.json".into(),
        super::locks::render(plan, &package)?,
    );
    files.insert(".fairway-install.lock".into(), Vec::new());
    files.insert(
        ".node-version".into(),
        format!("{}\n", plan.tools.node_version).into_bytes(),
    );
    files.insert(
        ".gitignore".into(),
        b"node_modules/\ndist/\n.env\n.env.*\n!.env.example\n*.log\n*.tsbuildinfo\n.DS_Store\n"
            .to_vec(),
    );
    files.insert(
        "LICENSE".into(),
        include_bytes!("../../../LICENSE").to_vec(),
    );
    let react = plan.template.id.starts_with("react-vite");
    files.insert(".env.example".into(),if react { b"# VITE_ values are public browser data; never put secrets here.\nVITE_APP_TITLE=My local app\n".to_vec() } else { b"# Nonsecret local defaults. Copy to .env to customize.\nHOST=127.0.0.1\nPORT=3000\n".to_vec() });
    let commands = plan
        .template
        .scripts
        .keys()
        .map(|key| format!("- `npm run {key}`\n"))
        .collect::<String>();
    files.insert("README.md".into(),format!("# {}\n\n{} — local template revision {}.\n\n## Setup\n\nCreated without automatically executing project scripts. A genuine, pinned npm lockfile is supplied. Dependencies are installed only through an explicitly confirmed installation action; inspect the installation job status. No application has been started.\n\nSelected Node: `{}`; paired npm: `{}`. `.node-version` records the choice without changing any global default.\n\nIf independently setting this project up in Terminal, review dependencies and installation scripts first; `npm ci --ignore-scripts` uses the reviewed dependency graph without install-time lifecycle hooks. No remote initializer is needed.\n\n## Development\n\n{}\nCopy `.env.example` to `.env` if needed. API starters load it explicitly; Vite exposes only public `VITE_` values. Never commit secrets.\n\nLocal address: http://127.0.0.1:{} . A running process does not establish readiness. Stop with Ctrl-C in Terminal or Stop in Fairway. Resolve port conflicts explicitly.\n\n## Provenance\n\nSources are Fairway-authored, under the included MIT license. `.fairway-template.json` records the catalog, asset revision and dependency references. Framework packages retain their own licenses when installed. No third-party package sources are bundled here.\n",plan.project_name,plan.template.title,plan.template.revision,plan.tools.node_version,plan.tools.npm_version,commands,if react {5173} else {3000}).into_bytes());
    files.insert(".fairway-template.json".into(),json(&serde_json::json!({"schemaVersion":1,"creationId":plan.id,"templateId":plan.template.id,"templateRevision":plan.template.revision,"catalogRevision":plan.catalog_revision,"assetRevision":revision(),"provenance":plan.template.provenance,"dependencies":plan.template.dependencies,"dependenciesInstalledByGeneration":false,"installationStatusSource":"Fairway creation journal","lockfile":"npmGenerated"})));
    if files.len() > 128 || files.values().map(Vec::len).sum::<usize>() > 2 * 1024 * 1024 {
        return Err(Error::new(
            ErrorCode::UnsafeTemplate,
            "Local template exceeds generation limits.",
        ));
    }
    for (name, bytes) in &files {
        validate_entry(name)?;
        if bytes.len() > 256 * 1024 {
            return Err(Error::new(
                ErrorCode::UnsafeTemplate,
                "Template file exceeds 256 KiB.",
            ));
        }
    }
    let names: std::collections::BTreeSet<_> = files.keys().collect();
    let intended: std::collections::BTreeSet<_> = plan.template.intended_files.iter().collect();
    if names != intended {
        return Err(Error::new(
            ErrorCode::UnsafeTemplate,
            "Catalog and materialized file list disagree.",
        ));
    }
    Ok(files)
}
