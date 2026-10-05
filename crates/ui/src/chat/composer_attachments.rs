//! Composer 附件状态与持久化（S05-07）。

use super::attachments::{self, DraftImage, MAX_IMAGE_COUNT};
use super::composer::Composer;
use crate::chat_bridge::spawn_engine;
use buddy_engine::models::ImageAttachment;
use gpui::{Context, PathPromptOptions};

impl Composer {
    /// 当前草稿图片（只读快照）。
    pub fn images(&self) -> Vec<ImageAttachment> {
        self.images
            .iter()
            .map(|image| image.attachment.clone())
            .collect()
    }

    /// 发送时取走图片草稿；调用方应在 `can_send` 成功后调用。
    pub fn take_images(&mut self, cx: &mut Context<Self>) -> Vec<ImageAttachment> {
        let images = std::mem::take(&mut self.images);
        self.attachment_error = None;
        cx.notify();
        images.into_iter().map(|image| image.attachment).collect()
    }

    /// 当前图片数量（自测与路由器使用）。
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// 当前附件错误（v1 InputDock 原文）。
    pub fn attachment_error(&self) -> Option<&str> {
        self.attachment_error.as_deref()
    }

    fn add_drafts(&mut self, mut images: Vec<DraftImage>, cx: &mut Context<Self>) {
        let slots = MAX_IMAGE_COUNT.saturating_sub(self.images.len());
        if slots == 0 {
            self.attachment_error = Some("每条消息最多添加 4 张图片".into());
            cx.notify();
            return;
        }
        if images.len() > slots {
            images.truncate(slots);
            self.attachment_error = Some("每条消息最多添加 4 张图片".into());
        } else {
            self.attachment_error = None;
        }
        let start = self.images.len();
        self.images.extend(images);
        self.saving_images = self.engine.is_some() && self.images.len() > start;
        let to_save: Vec<_> = self.images[start..].to_vec();
        let entity = cx.entity().downgrade();
        if let Some(engine) = self.engine.clone() {
            let saves = to_save
                .into_iter()
                .map(|image| {
                    let engine = engine.clone();
                    let save_image = image.clone();
                    (
                        image,
                        spawn_engine(cx, async move {
                            engine
                                .save_chat_image(
                                    save_image.attachment.name.clone(),
                                    save_image.attachment.media_type.clone(),
                                    save_image.attachment.data_url.clone(),
                                )
                                .await
                                .map(|mut saved| {
                                    // Engine paths can retain a sandbox `..`; delete
                                    // rejects those components, including orphan cleanup.
                                    if let Ok(path) = std::fs::canonicalize(&saved.path) {
                                        saved.path = path.to_string_lossy().into_owned();
                                    }
                                    saved
                                })
                        }),
                    )
                })
                .collect::<Vec<_>>();
            // The engine write outlives the view. Keep its completion alive too,
            // so closing the window cannot strand an unreferenced attachment.
            cx.spawn(async move |_, cx| {
                for (image, save) in saves {
                    let saved = save.await;
                    let orphan_path = saved.as_ref().ok().map(|image| image.path.clone());
                    let updated = entity.update(cx, |this, cx| {
                        if let Ok(saved) = saved {
                            if let Some(target) = this
                                .images
                                .iter_mut()
                                .find(|target| target.attachment.id == image.attachment.id)
                            {
                                target.attachment =
                                    attachments::persisted_attachment(&image, saved);
                            } else if !saved.path.is_empty() {
                                // 用户可能在保存完成前移除了缩略图；避免留下 engine 孤儿文件。
                                if let Some(engine) = this.engine.clone() {
                                    spawn_engine(cx, async move {
                                        engine.delete_chat_image(saved.path).await
                                    })
                                    .detach();
                                }
                            }
                        } else {
                            // 写盘失败时移除不可发送的临时附件，保留错误提示让用户重新选择。
                            this.images
                                .retain(|target| target.attachment.id != image.attachment.id);
                            this.attachment_error = Some("保存图片失败，请重试".into());
                        }
                        this.saving_images =
                            this.images.iter().any(|i| i.attachment.path.is_empty());
                        cx.notify();
                    });
                    if updated.is_err() {
                        if let Some(path) = orphan_path.filter(|path| !path.is_empty()) {
                            let cleanup_engine = engine.clone();
                            cx.update(|cx| spawn_engine(cx, async move { cleanup_engine.delete_chat_image(path).await })).detach();
                        }
                    }
                }
            }).detach();
        }
        cx.notify();
    }

    pub(super) fn remove_image(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(index) = self
            .images
            .iter()
            .position(|image| image.attachment.id == id)
        else {
            return;
        };
        let image = self.images.remove(index);
        if let (Some(engine), path) = (self.engine.clone(), image.attachment.path) {
            if !path.is_empty() {
                let path_task = async move { engine.delete_chat_image(path).await };
                spawn_engine(cx, path_task).detach();
            }
        }
        self.saving_images = self.engine.is_some() && self.images.iter().any(|i| i.attachment.path.is_empty());
        cx.notify();
    }

    pub(super) fn add_paths(&mut self, paths: &[std::path::PathBuf], cx: &mut Context<Self>) {
        let mut drafts = Vec::new();
        let mut first_error = None;
        let slots = MAX_IMAGE_COUNT.saturating_sub(self.images.len());
        let over_limit = paths.len() > slots;
        for path in paths.iter().take(slots) {
            match attachments::draft_from_path(path) {
                Ok(image) => drafts.push(image),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if !drafts.is_empty() {
            self.add_drafts(drafts, cx);
        }
        if over_limit {
            self.attachment_error = Some("每条消息最多添加 4 张图片".into());
        } else if let Some(error) = first_error {
            self.attachment_error = Some(error.into());
            cx.notify();
        }
        if over_limit {
            cx.notify();
        }
    }

    /// 打开平台原生文件选择器；读取结果仍归入拖放 / 剪贴板共用的校验入口。
    pub(super) fn pick_images(&mut self, cx: &mut Context<Self>) {
        if self.images.len() >= MAX_IMAGE_COUNT {
            self.attachment_error = Some("每条消息最多添加 4 张图片".into());
            cx.notify();
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("选择图片".into()),
        });
        let entity = cx.entity().downgrade();
        self.save_tasks.push(cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                let _ = entity.update(cx, |this, cx| this.add_paths(&paths, cx));
            }
        }));
    }

    /// 从系统剪贴板读取图片并加入草稿；返回是否消费了图片条目，文本由 `TextArea` 自己处理。
    pub fn paste_from_clipboard(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(item) = cx.read_from_clipboard() else {
            return false;
        };
        match attachments::draft_from_clipboard_image(&item) {
            Ok(Some(image)) => {
                self.add_drafts(vec![image], cx);
                true
            }
            Ok(None) => false,
            Err(error) => {
                self.attachment_error = Some(error.into());
                cx.notify();
                true
            }
        }
    }
}
