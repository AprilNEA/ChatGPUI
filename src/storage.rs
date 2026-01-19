// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

/// 获取应用数据目录
pub fn get_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ChatGPUI")
}

/// 获取附件存储目录
pub fn get_attachments_dir() -> PathBuf {
    get_data_dir().join("attachments")
}

/// 确保附件目录存在
pub async fn ensure_attachments_dir() -> Result<PathBuf> {
    let dir = get_attachments_dir();
    fs::create_dir_all(&dir).await?;
    Ok(dir)
}

/// 保存附件到文件系统
/// 返回相对于 attachments 目录的路径
pub async fn save_attachment(
    attachment_id: Uuid,
    data: &[u8],
    extension: &str,
) -> Result<String> {
    let dir = ensure_attachments_dir().await?;

    // 使用 attachment_id 作为文件名，避免冲突
    let filename = format!("{}.{}", attachment_id, extension);
    let file_path = dir.join(&filename);

    fs::write(&file_path, data).await?;

    Ok(filename)
}

/// 从文件系统加载附件
pub async fn load_attachment(relative_path: &str) -> Result<Vec<u8>> {
    let dir = get_attachments_dir();
    let file_path = dir.join(relative_path);
    let data = fs::read(&file_path).await?;
    Ok(data)
}

/// 删除附件文件
pub async fn delete_attachment(relative_path: &str) -> Result<()> {
    let dir = get_attachments_dir();
    let file_path = dir.join(relative_path);
    if file_path.exists() {
        fs::remove_file(&file_path).await?;
    }
    Ok(())
}

/// 从 MIME 类型获取文件扩展名
pub fn extension_from_mime(mime_type: &str) -> &str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}
