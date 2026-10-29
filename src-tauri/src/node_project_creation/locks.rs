//! npm-generated, pinned resolution graphs. No network resolution happens here.
use super::{CreationPlan, Error, ErrorCode, Result};
use base64::Engine;
pub fn raw(id: &str) -> Result<&'static [u8]> {
    Ok(match id {
        "node-js" => include_bytes!("../../templates/node-projects/locks/node-js.json"),
        "node-ts" => include_bytes!("../../templates/node-projects/locks/node-ts.json"),
        "hono-ts" => include_bytes!("../../templates/node-projects/locks/hono-ts.json"),
        "express-js" => include_bytes!("../../templates/node-projects/locks/express-js.json"),
        "react-vite-js" => include_bytes!("../../templates/node-projects/locks/react-vite-js.json"),
        "react-vite-ts" => include_bytes!("../../templates/node-projects/locks/react-vite-ts.json"),
        _ => {
            return Err(Error::new(
                ErrorCode::InvalidTemplate,
                "No reviewed lockfile exists for this template.",
            ))
        }
    })
}
pub fn revision() -> String {
    let mut data = include_bytes!("locks.rs").to_vec();
    data.extend_from_slice(include_bytes!(
        "../../templates/node-projects/locks/provenance.json"
    ));
    for id in [
        "node-js",
        "node-ts",
        "hono-ts",
        "express-js",
        "react-vite-js",
        "react-vite-ts",
    ] {
        data.extend_from_slice(raw(id).expect("known lock"));
    }
    super::digest(&data)
}
pub fn render(plan: &CreationPlan, package: &serde_json::Value) -> Result<Vec<u8>> {
    let mut lock: serde_json::Value =
        serde_json::from_slice(raw(&plan.template.id)?).map_err(|_| invalid())?;
    // Only package naming metadata changes. All resolved versions and integrity
    // entries remain exactly npm-generated; npm ci qualifies the normalized form.
    lock["name"] = plan.project_name.clone().into();
    lock["packages"][""]["name"] = plan.project_name.clone().into();
    validate(package, &lock)?;
    let mut bytes = serde_json::to_vec_pretty(&lock).map_err(|_| invalid())?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn invalid() -> Error {
    Error::new(ErrorCode::InvalidLockfile,"The reviewed npm lockfile is missing, malformed or inconsistent. No dependency upgrade/resolution was attempted.")
}
pub fn validate(package: &serde_json::Value, lock: &serde_json::Value) -> Result<()> {
    if lock["lockfileVersion"] != 3
        || lock["name"] != package["name"]
        || lock["version"] != package["version"]
    {
        return Err(invalid());
    }
    let packages = lock["packages"].as_object().ok_or_else(invalid)?;
    if packages.len() > 4096 {
        return Err(invalid());
    }
    let root = packages.get("").ok_or_else(invalid)?;
    for name in ["dependencies", "devDependencies"] {
        let empty = serde_json::json!({});
        let expected = package.get(name).unwrap_or(&empty);
        let actual = root.get(name).unwrap_or(&empty);
        if expected != actual {
            return Err(invalid());
        }
    }
    for (path, value) in packages {
        if path.is_empty() {
            continue;
        }
        if !path.starts_with("node_modules/")
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || path.contains('\\')
        {
            return Err(invalid());
        }
        let resolved = value["resolved"].as_str().ok_or_else(invalid)?;
        if !resolved.starts_with("https://registry.npmjs.org/")
            || resolved.chars().any(char::is_control)
            || resolved.contains('@') && !resolved.starts_with("https://registry.npmjs.org/@")
        {
            return Err(invalid());
        }
        let integrity = value["integrity"]
            .as_str()
            .and_then(|value| value.strip_prefix("sha512-"))
            .ok_or_else(invalid)?;
        if base64::engine::general_purpose::STANDARD
            .decode(integrity)
            .ok()
            .is_none_or(|bytes| bytes.len() != 64)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
