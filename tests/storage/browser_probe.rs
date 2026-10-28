//! Test-only WASM export; not linked into either application shell.
#[cfg(target_arch = "wasm32")]
mod probe {
    use feishin_storage::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn storage_probe(action: &str) -> Result<bool, JsValue> {
        let result = run(action).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(result)
    }
    fn run(action: &str) -> Result<bool, Error> {
        let mut repo = ProfileRepository::new(BrowserStore::open()?);
        match action {
            "defaults" => Ok(repo.load()?.data == ProfileData::default()),
            "save" => {
                let data = ProfileData {
                    preferences: Preferences {
                        language: "uk".into(),
                        follow_system_theme: true,
                    },
                    ..ProfileData::default()
                };
                repo.save(&data)?;
                Ok(repo.load()?.data == data)
            }
            "reload" => Ok(repo.load()?.data.preferences.language == "uk"),
            "partial" => {
                let loaded = repo.load()?;
                Ok(loaded.data.preferences == Preferences::default()
                    && loaded.recovered == vec![Recovery::Language])
            }
            "reject-save" => {
                repo.save(&ProfileData::default())?;
                Ok(true)
            }
            _ => Err(Error::InvalidDocument),
        }
    }
}
