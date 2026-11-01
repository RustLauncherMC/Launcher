//! Player heads for the account list: the face from the player's public skin, with its hat
//! layer, scaled up without blur. Skins come from Mojang's public profile service (no sign-in
//! needed) and heads are cached for a day.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{imageops, ImageFormat, RgbaImage};
use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

const PROFILE: &str = "https://sessionserver.mojang.com/session/minecraft/profile";
const MAX_SKIN: u64 = 1 << 20;
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);
/// The size heads are stored at; the UI scales them down.
const SIZE: u32 = 64;

fn cache_path(uuid: &str) -> PathBuf {
    crate::paths::cache_dir()
        .join("heads")
        .join(format!("{uuid}.png"))
}

fn valid_uuid(uuid: &str) -> bool {
    uuid.len() == 32 && uuid.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The player's head as a PNG, from the cache when it is recent.
pub fn head_png(agent: &ureq::Agent, uuid: &str) -> Result<Vec<u8>> {
    let uuid = uuid.replace('-', "").to_ascii_lowercase();
    if !valid_uuid(&uuid) {
        bail!("not a player UUID");
    }
    let path = cache_path(&uuid);
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().is_ok_and(|age| age < FRESH));
    if fresh {
        if let Ok(bytes) = std::fs::read(&path) {
            return Ok(bytes);
        }
    }
    let skin = download_skin(agent, &uuid)?;
    let png = head_from_skin(&skin)?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&path, &png);
    }
    Ok(png)
}

fn download_skin(agent: &ureq::Agent, uuid: &str) -> Result<Vec<u8>> {
    let profile: Value = agent
        .get(&format!("{PROFILE}/{uuid}"))
        .call()
        .context("could not reach Mojang")?
        .into_json()?;
    let url = skin_url(&profile).context("this player has no skin")?;
    let mut bytes = Vec::new();
    agent
        .get(&url)
        .call()
        .context("could not download the skin")?
        .into_reader()
        .take(MAX_SKIN)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The skin's URL from a profile's base64 `textures` property, only on Mojang's texture server
/// and always over HTTPS.
fn skin_url(profile: &Value) -> Option<String> {
    let encoded = profile["properties"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == "textures")?["value"]
        .as_str()?;
    let textures: Value = serde_json::from_slice(&STANDARD.decode(encoded).ok()?).ok()?;
    let url = textures["textures"]["SKIN"]["url"].as_str()?;
    let path = url
        .strip_prefix("http://textures.minecraft.net/")
        .or_else(|| url.strip_prefix("https://textures.minecraft.net/"))?;
    Some(format!("https://textures.minecraft.net/{path}"))
}

/// The face (8×8 at 8,8) with the hat layer (at 40,8) on top, scaled to [`SIZE`].
pub fn head_from_skin(skin_png: &[u8]) -> Result<Vec<u8>> {
    let skin = image::load_from_memory_with_format(skin_png, ImageFormat::Png)
        .context("the skin is not a PNG")?
        .to_rgba8();
    if skin.width() != 64 || !(skin.height() == 64 || skin.height() == 32) {
        bail!("the skin has an unexpected size");
    }
    let mut face: RgbaImage = imageops::crop_imm(&skin, 8, 8, 8, 8).to_image();
    let hat = imageops::crop_imm(&skin, 40, 8, 8, 8).to_image();
    // Like Minecraft, a hat layer with no transparent pixel is ignored: old skins often fill it
    // with opaque black, which would hide the face.
    if hat.pixels().any(|p| p.0[3] < 255) {
        imageops::overlay(&mut face, &hat, 0, 0);
    }
    let head = imageops::resize(&face, SIZE, SIZE, imageops::FilterType::Nearest);
    let mut png = Vec::new();
    head.write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cuts_the_face_and_hat_from_a_skin() {
        let mut skin = RgbaImage::new(64, 64);
        // A red face, with one green hat pixel at its top-left corner.
        for y in 8..16 {
            for x in 8..16 {
                skin.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        skin.put_pixel(40, 8, image::Rgba([0, 255, 0, 255]));
        let mut png = Vec::new();
        skin.write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let head = image::load_from_memory(&head_from_skin(&png).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(head.dimensions(), (SIZE, SIZE));
        assert_eq!(head.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(head.get_pixel(SIZE - 1, SIZE - 1).0, [255, 0, 0, 255]);
        assert!(head_from_skin(b"not a png").is_err());

        // An old 64×32 skin whose hat layer is opaque black: the face shows.
        let mut old = RgbaImage::new(64, 32);
        for y in 8..16 {
            for x in 8..16 {
                old.put_pixel(x, y, image::Rgba([200, 150, 120, 255]));
                old.put_pixel(x + 32, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        let mut png = Vec::new();
        old.write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let head = image::load_from_memory(&head_from_skin(&png).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(head.get_pixel(5, 5).0, [200, 150, 120, 255]);
    }

    #[test]
    fn skin_urls_stay_on_mojangs_server() {
        let profile = |url: &str| {
            let textures = json!({"textures": {"SKIN": {"url": url}}}).to_string();
            json!({"properties": [{"name": "textures", "value": STANDARD.encode(textures)}]})
        };
        assert_eq!(
            skin_url(&profile("http://textures.minecraft.net/texture/abc")).as_deref(),
            Some("https://textures.minecraft.net/texture/abc")
        );
        assert_eq!(skin_url(&profile("https://evil.example/texture/abc")), None);
        assert_eq!(skin_url(&json!({})), None);
    }
}
