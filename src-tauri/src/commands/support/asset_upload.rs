use super::*;

pub(in crate::commands) async fn upload_or_reuse_asset(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
    asset: &ProductAsset,
) -> AppResult<AssetUploadOutcome> {
    if let Some(error_summary) = validate_image_source_url(&asset.source_url) {
        record_asset_failure(app, item, asset, "INVALID_IMAGE_SOURCE_URL", &error_summary)?;
        mark_publish_item_failed_for_app(app, item, "INVALID_IMAGE_SOURCE_URL", &error_summary)?;
        return Ok(AssetUploadOutcome::Failed);
    }

    if is_wechat_image_url(&asset.source_url) {
        record_asset_success(app, item, asset, &asset.source_url, "reused")?;
        return Ok(AssetUploadOutcome::Reused);
    }

    if let Some(wechat_url) = find_cached_asset_url(app, &item.shop_id, &asset.source_url)? {
        record_asset_success(app, item, asset, &wechat_url, "reused")?;
        return Ok(AssetUploadOutcome::Reused);
    }

    let prepared = match prepare_image_for_upload(app, &asset.source_url).await {
        Ok(prepared) => prepared,
        Err(error_summary) => {
            record_asset_failure(app, item, asset, "IMAGE_PREPROCESS_FAILED", &error_summary)?;
            mark_publish_item_failed_for_app(app, item, "IMAGE_PREPROCESS_FAILED", &error_summary)?;
            return Ok(AssetUploadOutcome::Failed);
        }
    };

    let call = match client
        .upload_image_bytes(
            access_token,
            prepared.bytes,
            &prepared.file_name,
            prepared.mime_type,
            prepared.width,
            prepared.height,
        )
        .await
    {
        Ok(call) => call,
        Err(error) => {
            let error_summary = format!("微信图片上传请求失败：{error}");
            record_asset_failure(
                app,
                item,
                asset,
                "WECHAT_IMAGE_UPLOAD_HTTP_FAILED",
                &error_summary,
            )?;
            mark_publish_item_failed_for_app(
                app,
                item,
                "WECHAT_IMAGE_UPLOAD_HTTP_FAILED",
                &error_summary,
            )?;
            return Ok(AssetUploadOutcome::Failed);
        }
    };

    let conn = open_connection(app)?;
    match &call.result {
        WechatCallResult::Success(result) => {
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!("image upload ok, {}", prepared.summary)),
            )?;
            drop(conn);
            record_asset_success(app, item, asset, &result.img_url, "success")?;
            insert_task_log_for_app(
                app,
                &item.job_id,
                Some(&item.item_id),
                "info",
                &format!("素材预处理完成：{}", prepared.summary),
                Some(&serde_json::json!({
                    "asset_kind": asset.kind,
                    "sort_order": asset.sort_order,
                    "local_path": prepared.local_path.display().to_string()
                })),
            )?;
            Ok(AssetUploadOutcome::Uploaded)
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("image upload api error"),
            )?;
            drop(conn);
            let error_code = format!("WECHAT_IMAGE_UPLOAD_{}", error.errcode);
            let error_summary = format!("微信图片上传失败：{}", error.errmsg);
            record_asset_failure(app, item, asset, &error_code, &error_summary)?;
            mark_publish_item_failed_for_app(app, item, &error_code, &error_summary)?;
            Ok(AssetUploadOutcome::Failed)
        }
    }
}

pub(in crate::commands) fn validate_image_source_url(source_url: &str) -> Option<String> {
    let parsed = match Url::parse(source_url) {
        Ok(parsed) => parsed,
        Err(error) => return Some(format!("图片 URL 不合法：{error}")),
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Some("图片 URL 必须使用 http 或 https".to_string());
    }
    if parsed.host_str().is_none() {
        return Some("图片 URL 缺少域名".to_string());
    }
    if parsed.path().contains("//") {
        return Some("图片 URL 路径不能包含连续 //，微信图片接口不支持该格式".to_string());
    }
    if let Some(host) = parsed.host_str().map(|host| host.to_ascii_lowercase()) {
        if host == "localhost" || host.ends_with(".localhost") || host == "127.0.0.1" {
            return Some("图片 URL 不能指向 localhost 或 127.0.0.1".to_string());
        }
    }
    None
}

pub(in crate::commands) async fn prepare_image_for_upload(
    app: &AppHandle,
    source_url: &str,
) -> Result<PreparedImageUpload, String> {
    let http = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(StdDuration::from_secs(IMAGE_DOWNLOAD_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| format!("初始化图片下载客户端失败：{error}"))?;
    let response = http
        .get(source_url)
        .header(USER_AGENT, IMAGE_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("下载图片失败：{error}"))?;
    let status = response.status();
    if status.is_redirection() {
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        return Err(if location.is_empty() {
            format!("图片 URL 返回 {status} 跳转，微信 URL 上传不支持 301/302")
        } else {
            format!("图片 URL 返回 {status} 跳转到 {location}，微信 URL 上传不支持 301/302")
        });
    }
    if !status.is_success() {
        return Err(format!("图片 URL 打开失败：HTTP {status}"));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or(value)
                .trim()
                .to_ascii_lowercase()
        });
    if matches!(content_type.as_deref(), Some("text/xml" | "image/svg+xml")) {
        return Err("SVG 暂不支持本地规范化，请先转为 PNG/JPEG 再铺货".to_string());
    }
    if let Some(content_length) = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        if content_length > IMAGE_DOWNLOAD_MAX_BYTES {
            return Err(format!(
                "图片过大：{}，超过本地下载上限 {}",
                format_bytes_short(content_length as usize),
                format_bytes_short(IMAGE_DOWNLOAD_MAX_BYTES as usize)
            ));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取图片内容失败：{error}"))?;
    if bytes.is_empty() {
        return Err("图片内容为空".to_string());
    }
    if bytes.len() > IMAGE_DOWNLOAD_MAX_BYTES as usize {
        return Err(format!(
            "图片过大：{}，超过本地下载上限 {}",
            format_bytes_short(bytes.len()),
            format_bytes_short(IMAGE_DOWNLOAD_MAX_BYTES as usize)
        ));
    }

    let format =
        image::guess_format(&bytes).map_err(|error| format!("图片格式无法识别：{error}"))?;
    let image = image::load_from_memory_with_format(&bytes, format)
        .map_err(|error| format!("图片解码失败：{error}"))?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("图片宽高无效".to_string());
    }

    let (upload_bytes, upload_width, upload_height, mime_type, extension, summary_suffix) =
        if let Some(mime_type) = original_upload_mime(format) {
            if bytes.len() <= WECHAT_IMAGE_MAX_BYTES {
                (
                    bytes.to_vec(),
                    width,
                    height,
                    mime_type,
                    original_upload_extension(format).unwrap_or("img"),
                    format!("保留原格式 {}", image_format_label(format)),
                )
            } else {
                let normalized = encode_jpeg_under_limit(image)?;
                let summary_suffix = format!(
                    "已压缩转 JPEG，{} -> {}",
                    format_bytes_short(bytes.len()),
                    format_bytes_short(normalized.0.len())
                );
                (
                    normalized.0,
                    normalized.1,
                    normalized.2,
                    "image/jpeg",
                    "jpg",
                    summary_suffix,
                )
            }
        } else {
            let normalized = encode_jpeg_under_limit(image)?;
            let summary_suffix = format!(
                "{} 已转 JPEG，{} -> {}",
                image_format_label(format),
                format_bytes_short(bytes.len()),
                format_bytes_short(normalized.0.len())
            );
            (
                normalized.0,
                normalized.1,
                normalized.2,
                "image/jpeg",
                "jpg",
                summary_suffix,
            )
        };

    if upload_bytes.len() > WECHAT_IMAGE_MAX_BYTES {
        return Err(format!(
            "图片规范化后仍超过微信 10MB 限制：{}",
            format_bytes_short(upload_bytes.len())
        ));
    }

    let hash = hex_sha256(source_url.as_bytes());
    let dir = image_cache_dir(app).map_err(|error| format!("创建图片缓存目录失败：{error}"))?;
    let local_path = dir.join(format!("{hash}.{extension}"));
    fs::write(&local_path, &upload_bytes).map_err(|error| format!("写入图片缓存失败：{error}"))?;
    let file_name = local_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("wx-xd-image.jpg")
        .to_string();
    let summary = format!(
        "{}x{}，{}，{}",
        upload_width,
        upload_height,
        format_bytes_short(upload_bytes.len()),
        summary_suffix
    );

    Ok(PreparedImageUpload {
        bytes: upload_bytes,
        width: upload_width,
        height: upload_height,
        mime_type,
        file_name,
        local_path,
        summary,
    })
}

pub(in crate::commands) fn image_cache_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app.path().app_data_dir()?.join("image-cache");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(in crate::commands) fn original_upload_mime(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Png => Some("image/png"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}

pub(in crate::commands) fn original_upload_extension(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("jpg"),
        ImageFormat::Png => Some("png"),
        ImageFormat::WebP => Some("webp"),
        ImageFormat::Bmp => Some("bmp"),
        _ => None,
    }
}

pub(in crate::commands) fn image_format_label(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Png => "PNG",
        ImageFormat::WebP => "WEBP",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Gif => "GIF",
        _ => "未知格式",
    }
}

pub(in crate::commands) fn encode_jpeg_under_limit(
    image: DynamicImage,
) -> Result<(Vec<u8>, u32, u32), String> {
    let mut last_bytes = Vec::new();
    let mut last_width = 0u32;
    let mut last_height = 0u32;
    for max_side in [2400u32, 2000, 1600, 1200, 900] {
        let working = resize_to_max_side(&image, max_side);
        let (width, height) = working.dimensions();
        for quality in [86u8, 78, 70, 62] {
            let encoded = encode_jpeg_with_white_background(&working, quality)?;
            if encoded.len() <= WECHAT_IMAGE_TARGET_BYTES {
                return Ok((encoded, width, height));
            }
            last_bytes = encoded;
            last_width = width;
            last_height = height;
        }
    }
    if last_bytes.len() <= WECHAT_IMAGE_MAX_BYTES {
        Ok((last_bytes, last_width, last_height))
    } else {
        Err(format!(
            "图片压缩后仍过大：{}",
            format_bytes_short(last_bytes.len())
        ))
    }
}

pub(in crate::commands) fn resize_to_max_side(image: &DynamicImage, max_side: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width <= max_side && height <= max_side {
        return image.clone();
    }
    image.resize(max_side, max_side, image::imageops::FilterType::Lanczos3)
}

pub(in crate::commands) fn encode_jpeg_with_white_background(
    image: &DynamicImage,
    quality: u8,
) -> Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(width, height);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = pixel[3] as u16;
        let blend = |channel: u8| -> u8 {
            (((channel as u16 * alpha) + (255u16 * (255 - alpha))) / 255) as u8
        };
        rgb.put_pixel(
            x,
            y,
            image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
        );
    }
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
    encoder
        .encode(&rgb, width, height, image::ColorType::Rgb8.into())
        .map_err(|error| format!("图片转 JPEG 失败：{error}"))?;
    Ok(bytes)
}

pub(in crate::commands) fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(in crate::commands) fn format_bytes_short(value: usize) -> String {
    if value >= 1024 * 1024 {
        format!("{:.2}MB", value as f64 / 1024.0 / 1024.0)
    } else if value >= 1024 {
        format!("{:.2}KB", value as f64 / 1024.0)
    } else {
        format!("{value}B")
    }
}

pub(in crate::commands) fn is_wechat_image_url(source_url: &str) -> bool {
    Url::parse(source_url)
        .ok()
        .map(|url| {
            url.host_str()
                .map(|host| host == "mmecimage.cn" || host.ends_with(".mmecimage.cn"))
                .unwrap_or(false)
                && url.path().starts_with("/p/")
        })
        .unwrap_or(false)
}

pub(in crate::commands) fn find_cached_asset_url(
    app: &AppHandle,
    shop_id: &str,
    source_url: &str,
) -> AppResult<Option<String>> {
    let conn = open_connection(app)?;
    let wechat_url = conn
        .query_row(
            "SELECT wechat_url
             FROM publish_assets
             WHERE shop_id = ?1
               AND source_url = ?2
               AND status IN ('success', 'reused')
               AND wechat_url IS NOT NULL
             ORDER BY uploaded_at DESC, updated_at DESC
             LIMIT 1",
            params![shop_id, source_url],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(wechat_url)
}

pub(in crate::commands) fn record_asset_success(
    app: &AppHandle,
    item: &PendingPublishItem,
    asset: &ProductAsset,
    wechat_url: &str,
    status: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO publish_assets
         (id, job_id, item_id, product_row_id, shop_id, source_url, asset_kind, sort_order,
          wechat_url, status, uploaded_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?11)
         ON CONFLICT(item_id, source_url) DO UPDATE SET
           asset_kind = excluded.asset_kind,
           sort_order = excluded.sort_order,
           wechat_url = excluded.wechat_url,
           status = excluded.status,
           error_code = NULL,
           error_summary = NULL,
           uploaded_at = excluded.uploaded_at,
           updated_at = excluded.updated_at",
        params![
            format!("asset-{}", Uuid::new_v4()),
            item.job_id.as_str(),
            item.item_id.as_str(),
            item.product_row_id.as_str(),
            item.shop_id.as_str(),
            asset.source_url.as_str(),
            asset.kind,
            asset.sort_order,
            wechat_url,
            status,
            now
        ],
    )?;
    insert_task_log(
        &conn,
        &item.job_id,
        Some(&item.item_id),
        "info",
        "素材已准备为微信图片链接",
        Some(&serde_json::json!({
            "asset_kind": asset.kind,
            "sort_order": asset.sort_order,
            "status": status
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn record_asset_failure(
    app: &AppHandle,
    item: &PendingPublishItem,
    asset: &ProductAsset,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO publish_assets
         (id, job_id, item_id, product_row_id, shop_id, source_url, asset_kind, sort_order,
          status, error_code, error_summary, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'failed', ?9, ?10, ?11, ?11)
         ON CONFLICT(item_id, source_url) DO UPDATE SET
           asset_kind = excluded.asset_kind,
           sort_order = excluded.sort_order,
           status = 'failed',
           error_code = excluded.error_code,
           error_summary = excluded.error_summary,
           updated_at = excluded.updated_at",
        params![
            format!("asset-{}", Uuid::new_v4()),
            item.job_id.as_str(),
            item.item_id.as_str(),
            item.product_row_id.as_str(),
            item.shop_id.as_str(),
            asset.source_url.as_str(),
            asset.kind,
            asset.sort_order,
            error_code,
            error_summary,
            now
        ],
    )?;
    insert_task_log(
        &conn,
        &item.job_id,
        Some(&item.item_id),
        "error",
        error_summary,
        Some(&serde_json::json!({
            "error_code": error_code,
            "asset_kind": asset.kind,
            "sort_order": asset.sort_order
        })),
    )?;
    Ok(())
}
