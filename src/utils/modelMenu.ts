import type { ModelInfo, ProviderConfig } from '@/types';

let activeModelMenu: Awaited<
  ReturnType<typeof import('@tauri-apps/api/menu').Menu.new>
> | null = null;

interface OpenModelMenuOptions {
  models: ModelInfo[];
  providers: ProviderConfig[];
  selectedId: string;
  onSelect: (modelId: string) => void | Promise<void>;
}

/**
 * 使用系统原生菜单展示模型选项。
 *
 * 原生菜单独立于 WebView 窗口绘制，因此紧凑气泡无需扩高来容纳下拉内容。
 * 浏览器预览环境返回 false，由调用方使用页面内下拉作为回退。
 */
export async function openNativeModelMenu({
  models,
  providers,
  selectedId,
  onSelect,
}: OpenModelMenuOptions): Promise<boolean> {
  if (typeof window === 'undefined' || !(window as any).__TAURI_INTERNALS__) {
    return false;
  }

  const [{ CheckMenuItem, Menu }, { getCurrentWindow }] = await Promise.all([
    import('@tauri-apps/api/menu'),
    import('@tauri-apps/api/window'),
  ]);

  await activeModelMenu?.close();
  const providerNames = new Map(providers.map((provider) => [provider.id, provider.name]));

  const items = await Promise.all(
    models.map((model, index) =>
      CheckMenuItem.new({
        id: `buddy-model-${index}`,
        text: providerNames.has(model.provider_id)
          ? `${model.display_name} · ${providerNames.get(model.provider_id)}`
          : model.display_name,
        checked: model.id === selectedId,
        action: () => {
          void onSelect(model.id);
        },
      }),
    ),
  );

  activeModelMenu = await Menu.new({ items });
  await activeModelMenu.popup(undefined, getCurrentWindow());
  return true;
}
