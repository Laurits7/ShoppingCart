//! Pictures (SPEC §5.2, §7.4): uploads and downloaded product photos are decoded with size
//! and memory limits, turned upright, resized to a thumbnail and a large picture and
//! re-encoded as JPEG (which drops EXIF/GPS), then stored under `data/images/` by the SHA-256
//! of the original. The original is never kept.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbImage,
    codecs::jpeg::JpegEncoder, imageops::FilterType,
};
use sha2::{Digest, Sha256};
use shoppingcart_domain::images::{self, Kind, LARGE_PX, MAX_SIDE_PX, THUMB_PX};

use crate::{
    AppState,
    error::{ApiResult, AppError, bad},
    models::Image,
    util::{new_id, now},
};

/// Decoding a big photo takes real memory and CPU: one at a time.
static PROCESSING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
/// Most memory a single decode may allocate.
const MAX_ALLOC: u64 = 160 * 1024 * 1024;

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("images")
}

pub fn file(data_dir: &Path, sha256: &str, large: bool) -> PathBuf {
    dir(data_dir).join(if large {
        format!("{sha256}-l.jpg")
    } else {
        format!("{sha256}.jpg")
    })
}

/// The two JPEGs and the large picture's size.
struct Processed {
    thumb: Vec<u8>,
    large: Vec<u8>,
    width: u32,
    height: u32,
}

fn encode(img: &DynamicImage, max: u32, quality: u8) -> Result<(Vec<u8>, u32, u32), String> {
    let (w, h) = images::fit(img.width(), img.height(), max);
    let small = if (w, h) == (img.width(), img.height()) {
        img.clone()
    } else {
        img.resize_exact(w, h, FilterType::Triangle)
    };
    // Transparent parts become white (JPEG has no transparency).
    let rgba = small.to_rgba8();
    let rgb = RgbImage::from_fn(w, h, |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u32;
        let blend = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
        image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&rgb)
        .map_err(|e| format!("could not encode the picture: {e}"))?;
    Ok((out, w, h))
}

fn process(bytes: &[u8]) -> Result<Processed, String> {
    let format = match images::sniff(bytes) {
        Some(Kind::Jpeg) => ImageFormat::Jpeg,
        Some(Kind::Png) => ImageFormat::Png,
        Some(Kind::WebP) => ImageFormat::WebP,
        None => return Err("only JPEG, PNG and WebP pictures are accepted".into()),
    };
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE_PX);
    limits.max_image_height = Some(MAX_SIDE_PX);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let unreadable = |e: image::ImageError| match e {
        image::ImageError::Limits(_) => "the picture is too big".to_string(),
        _ => "the picture could not be read".to_string(),
    };
    let mut decoder = reader.into_decoder().map_err(unreadable)?;
    // Phones store photos sideways plus a rotation tag; apply it before the tag is dropped.
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(unreadable)?;
    img.apply_orientation(orientation);
    let (thumb, _, _) = encode(&img, THUMB_PX, 80)?;
    let (large, width, height) = encode(&img, LARGE_PX, 82)?;
    Ok(Processed {
        thumb,
        large,
        width,
        height,
    })
}

/// Write a file so a crash never leaves a half-written picture behind.
async fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", new_id()));
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await
}

pub fn sha256_bytes(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const COLS: &str = "id, width, height, source, source_ref, created_at";

pub async fn load(state: &AppState, id: &str) -> sqlx::Result<Option<Image>> {
    sqlx::query_as(&format!("SELECT {COLS} FROM images WHERE id = ?"))
        .bind(id)
        .fetch_optional(&state.db.read)
        .await
}

/// Store a picture for `owner`: the same picture again returns the existing one.
pub async fn store(
    state: &AppState,
    owner: &str,
    bytes: Vec<u8>,
    source: &str,
    source_ref: Option<&str>,
) -> ApiResult<Image> {
    if bytes.len() > state.config.max_upload_bytes {
        return Err(bad(format!(
            "the picture is larger than {} MB",
            state.config.max_upload_bytes / 1024 / 1024
        )));
    }
    images::sniff(&bytes).ok_or_else(|| bad("only JPEG, PNG and WebP pictures are accepted"))?;
    let sha = sha256_bytes(&bytes);
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM images WHERE owner_id = ? AND sha256 = ?")
            .bind(owner)
            .bind(&sha)
            .fetch_optional(&state.db.read)
            .await?;
    if let Some(id) = existing {
        return load(state, &id).await?.ok_or(AppError::NotFound);
    }
    // Someone else may have stored the same picture already: reuse the files.
    let known: Option<(i64, i64, i64)> =
        sqlx::query_as("SELECT width, height, bytes FROM images WHERE sha256 = ? LIMIT 1")
            .bind(&sha)
            .fetch_optional(&state.db.read)
            .await?;
    let data_dir = &state.config.data_dir;
    let (width, height, size) = match known {
        Some(k) if file(data_dir, &sha, true).exists() && file(data_dir, &sha, false).exists() => k,
        _ => {
            let _permit = PROCESSING.acquire().await?;
            let p = tokio::task::spawn_blocking(move || process(&bytes))
                .await?
                .map_err(bad)?;
            tokio::fs::create_dir_all(dir(data_dir)).await?;
            write_atomic(&file(data_dir, &sha, false), &p.thumb).await?;
            write_atomic(&file(data_dir, &sha, true), &p.large).await?;
            (
                p.width as i64,
                p.height as i64,
                (p.thumb.len() + p.large.len()) as i64,
            )
        }
    };
    let id = new_id();
    sqlx::query(
        "INSERT INTO images (id, owner_id, sha256, width, height, bytes, source, source_ref, created_at)
         VALUES (?,?,?,?,?,?,?,?,?)
         ON CONFLICT (owner_id, sha256) DO NOTHING",
    )
    .bind(&id)
    .bind(owner)
    .bind(&sha)
    .bind(width)
    .bind(height)
    .bind(size)
    .bind(source)
    .bind(source_ref)
    .bind(now())
    .execute(&state.db.write)
    .await?;
    // A parallel upload of the same picture may have won the insert.
    let id: String = sqlx::query_scalar("SELECT id FROM images WHERE owner_id = ? AND sha256 = ?")
        .bind(owner)
        .bind(&sha)
        .fetch_one(&state.db.write)
        .await?;
    load(state, &id).await?.ok_or(AppError::NotFound)
}

/// Forget pictures nothing uses any more (a day after upload, so a picture being attached
/// isn't lost), and their files once no row needs them.
pub async fn purge_orphans(state: &AppState) -> anyhow::Result<()> {
    let day_ago = (chrono::Utc::now() - chrono::Duration::days(1))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let gone: Vec<String> = sqlx::query_scalar(
        "DELETE FROM images WHERE created_at < ?
           AND id NOT IN (SELECT image_id FROM products WHERE image_id IS NOT NULL)
           AND id NOT IN (SELECT image_id FROM catalog_items WHERE image_id IS NOT NULL)
         RETURNING sha256",
    )
    .bind(day_ago)
    .fetch_all(&state.db.write)
    .await?;
    for sha in gone {
        let still: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM images WHERE sha256 = ? LIMIT 1")
                .bind(&sha)
                .fetch_optional(&state.db.read)
                .await?;
        if still.is_none() {
            for large in [false, true] {
                let _ = tokio::fs::remove_file(file(&state.config.data_dir, &sha, large)).await;
            }
        }
    }
    Ok(())
}

/// The bytes of a stored picture (the caller has checked access).
pub async fn read(state: &AppState, id: &str, large: bool) -> ApiResult<Vec<u8>> {
    let sha: String = sqlx::query_scalar("SELECT sha256 FROM images WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db.read)
        .await?
        .ok_or(AppError::NotFound)?;
    tokio::fs::read(file(&state.config.data_dir, &sha, large))
        .await
        .map_err(|_| AppError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img =
            image::RgbaImage::from_fn(w, h, |x, _| image::Rgba([(x % 256) as u8, 10, 200, 128]));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn resizes_and_reencodes() {
        let p = process(&png(1600, 800)).unwrap();
        assert_eq!((p.width, p.height), (640, 320));
        assert_eq!(images::sniff(&p.thumb), Some(Kind::Jpeg));
        assert_eq!(images::sniff(&p.large), Some(Kind::Jpeg));
        let t = image::load_from_memory(&p.thumb).unwrap();
        assert_eq!((t.width(), t.height()), (160, 80));
        // Small pictures are not enlarged.
        let p = process(&png(50, 40)).unwrap();
        assert_eq!((p.width, p.height), (50, 40));
    }

    #[test]
    fn refuses_other_things() {
        assert!(process(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_err());
        // A truncated PNG.
        let mut b = png(100, 100);
        b.truncate(60);
        assert!(process(&b).is_err());
    }
}
