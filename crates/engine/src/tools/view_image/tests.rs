use super::*;
use crate::storage;

fn vision_context() -> ToolContext {
    ToolContext {
        supports_vision: true,
        ..ToolContext::default()
    }
}

fn stored_image(directory: &std::path::Path) -> ImageAttachment {
    storage::store_image_bytes(
        directory,
        "picture.png",
        "image/png",
        b"\x89PNG\r\n\x1a\nimage",
        "test",
    )
    .unwrap()
}

#[tokio::test]
async fn only_registered_attachment_ids_can_be_viewed() {
    let directory = tempfile::tempdir().unwrap();
    let image = stored_image(directory.path());
    let tool = ViewImageTool::new(directory.path().to_path_buf());

    for args in [
        json!({"path": image.path}),
        json!({"attachment_id": image.id}),
    ] {
        assert!(matches!(
            tool.execute(args, vision_context()).await,
            Err(ToolError::InvalidArgs(_))
        ));
    }
    tool.remember_images(std::slice::from_ref(&image));
    let output = tool
        .execute(json!({"attachment_id": image.id}), vision_context())
        .await
        .unwrap();
    assert_eq!(output.model_images.len(), 1);
    assert_eq!(output.model_images[0].id, image.id);
    assert!(
        output.model_images[0]
            .data_url
            .starts_with("data:image/png;base64,")
    );

    // 新对话不能凭上一轮的 ID 或已存在的附件文件获得访问。
    let other = ViewImageTool::new(directory.path().to_path_buf());
    assert!(
        other
            .execute(json!({"attachment_id": image.id}), vision_context())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn text_disguised_as_image_is_not_returned() {
    let directory = tempfile::tempdir().unwrap();
    let image = stored_image(directory.path());
    std::fs::write(&image.path, b"private text").unwrap();
    let tool = ViewImageTool::new(directory.path().to_path_buf());
    tool.remember_images(std::slice::from_ref(&image));
    let error = tool
        .execute(json!({"attachment_id": image.id}), vision_context())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("图片格式"));
    assert!(!error.to_string().contains("private text"));
}

#[tokio::test]
async fn registered_image_cannot_escape_attachment_directory() {
    let directory = tempfile::tempdir().unwrap();
    let mut image = stored_image(directory.path());
    let outside = directory.path().join("private.png");
    std::fs::copy(&image.path, &outside).unwrap();
    image.path = outside.to_string_lossy().into_owned();
    let tool = ViewImageTool::new(directory.path().to_path_buf());
    tool.remember_images(std::slice::from_ref(&image));
    assert!(matches!(
        tool.execute(json!({"attachment_id": image.id}), vision_context())
            .await,
        Err(ToolError::PermissionDenied(_))
    ));

    #[cfg(unix)]
    {
        let link = directory.path().join("attachments/linked.png");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        image.path = link.to_string_lossy().into_owned();
        tool.remember_images(std::slice::from_ref(&image));
        assert!(matches!(
            tool.execute(json!({"attachment_id": image.id}), vision_context())
                .await,
            Err(ToolError::PermissionDenied(_))
        ));
    }
}

#[tokio::test]
async fn vision_and_cancellation_are_checked_before_reading() {
    let directory = tempfile::tempdir().unwrap();
    let image = stored_image(directory.path());
    let tool = ViewImageTool::new(directory.path().to_path_buf());
    tool.remember_images(std::slice::from_ref(&image));
    let args = json!({"attachment_id": image.id});
    assert!(
        tool.execute(args.clone(), ToolContext::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("视觉")
    );
    std::fs::remove_file(&image.path).unwrap();
    let (_tx, rx) = tokio::sync::watch::channel(true);
    let context = ToolContext {
        cancel_rx: Some(rx),
        ..vision_context()
    };
    assert!(
        tool.execute(args, context)
            .await
            .unwrap_err()
            .to_string()
            .contains("取消")
    );
}
