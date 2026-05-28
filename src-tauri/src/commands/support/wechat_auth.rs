use super::*;

pub(in crate::commands) fn upsert_shop_secret(
    app: &AppHandle,
    conn: &Connection,
    shop_id: &str,
    secret: &str,
    updated_at: &str,
) -> AppResult<()> {
    let encrypted = encrypt_secret(app, secret)?;
    let fingerprint = secret_fingerprint(secret);
    conn.execute(
        "INSERT INTO shop_credentials
         (shop_id, encrypted_secret, secret_nonce, key_version, secret_fingerprint, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(shop_id) DO UPDATE SET
           encrypted_secret = excluded.encrypted_secret,
           secret_nonce = excluded.secret_nonce,
           key_version = excluded.key_version,
           secret_fingerprint = excluded.secret_fingerprint,
           updated_at = excluded.updated_at",
        params![
            shop_id,
            encrypted.ciphertext,
            encrypted.nonce,
            encrypted.key_version,
            fingerprint,
            updated_at
        ],
    )?;
    Ok(())
}

pub(in crate::commands) async fn ensure_access_token(
    app: &AppHandle,
    shop_id: &str,
    client: &WechatShopClient,
) -> AppResult<String> {
    let cached_token = {
        let conn = open_connection(app)?;
        conn.query_row(
            "SELECT encrypted_access_token, token_nonce, expires_at
             FROM access_tokens
             WHERE shop_id = ?1",
            [shop_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
    };
    if let Some((encrypted_access_token, token_nonce, expires_at)) = cached_token {
        if is_future_rfc3339(&expires_at) {
            return decrypt_access_token(app, &encrypted_access_token, &token_nonce);
        }
    }

    let (appid, encrypted_secret, secret_nonce) = {
        let conn = open_connection(app)?;
        conn.query_row(
            "SELECT s.appid, c.encrypted_secret, c.secret_nonce
             FROM shops s
             JOIN shop_credentials c ON c.shop_id = s.id
             WHERE s.id = ?1",
            [shop_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("店铺不存在或未保存 app_secret".to_string()))?
    };
    let secret = decrypt_secret(app, &encrypted_secret, &secret_nonce)?;
    let call = client
        .get_stable_access_token(&appid, &secret, false)
        .await?;
    let conn = open_connection(app)?;
    match &call.result {
        WechatCallResult::Success(token) => {
            let encrypted_token = encrypt_access_token(app, &token.access_token)?;
            let expires_at = expires_at_shanghai(token.expires_in);
            let refreshed_at = now_shanghai();
            conn.execute(
                "INSERT INTO access_tokens
                 (shop_id, encrypted_access_token, token_nonce, key_version, expires_at, refreshed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(shop_id) DO UPDATE SET
                   encrypted_access_token = excluded.encrypted_access_token,
                   token_nonce = excluded.token_nonce,
                   key_version = excluded.key_version,
                   expires_at = excluded.expires_at,
                   refreshed_at = excluded.refreshed_at",
                params![
                    shop_id,
                    encrypted_token.ciphertext,
                    encrypted_token.nonce,
                    encrypted_token.key_version,
                    expires_at,
                    refreshed_at
                ],
            )?;
            insert_api_call_log(
                &conn,
                Some(shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "stable_token refreshed, expires_in={}s",
                    token.expires_in
                )),
            )?;
            Ok(token.access_token.clone())
        }
        WechatCallResult::ApiError(error) => {
            conn.execute(
                "UPDATE shops SET status = 'auth_failed' WHERE id = ?1",
                [shop_id],
            )?;
            insert_api_call_log(
                &conn,
                Some(shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("stable_token refresh api error"),
            )?;
            Err(AppError::WechatApi {
                errcode: error.errcode,
                errmsg: error.errmsg.clone(),
            })
        }
    }
}

pub(in crate::commands) fn insert_api_call_log(
    conn: &Connection,
    shop_id: Option<&str>,
    endpoint: &str,
    method: &str,
    status: &str,
    errcode: Option<i64>,
    errmsg: Option<&str>,
    response_summary: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO api_call_logs
         (id, shop_id, provider, endpoint, method, status, errcode, errmsg, response_summary, created_at)
         VALUES (?1, ?2, 'wechat_shop', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            format!("api-{}", Uuid::new_v4()),
            shop_id,
            endpoint,
            method,
            status,
            errcode,
            errmsg,
            response_summary,
            now_shanghai()
        ],
    )?;
    Ok(())
}
