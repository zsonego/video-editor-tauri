// false：直接使用安装包内的 DLL；true：启动时检查并按需下载运行库。
export const WINDOWS_RUNTIME_AUTO_DOWNLOAD_ENABLED =
  String(
    import.meta.env.VITE_WINDOWS_RUNTIME_AUTO_DOWNLOAD_ENABLED || 'false',
  )
    .trim()
    .toLowerCase() === 'true';

export const WINDOWS_RUNTIME_VERSION = String(
  import.meta.env.VITE_WINDOWS_RUNTIME_VERSION || '2026-09-09',
).trim();

export const WINDOWS_RUNTIME_DOWNLOAD_PATH = 'aicutx/dll/win.zip';

export function encodeBase64Url(value) {
  const bytes = new TextEncoder().encode(String(value || ''));
  let binary = '';
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary)
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/g, '');
}
